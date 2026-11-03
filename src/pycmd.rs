//! Python commands: the contract shared by both execution modes.
//!
//! A Python command returns exactly what a Rust command returns, so callers
//! (flows, newbound's `format_result`, MCP tools) never special-case the
//! language:
//!
//! - success: `{"a": <return value>}`
//! - failure: `{"a": {"status": "err", "msg": <report>}}`, where the report
//!   is the exception, the location in the command's own body
//!   ("body line N"), and the full traceback.
//!
//! With the `python_runtime` feature the command runs in the embedded
//! interpreter (see `pyenv`), receiving `NDataObject`/`NDataArray`/
//! `NDataBytes` handles. Without it, the command runs in a `python3`
//! subprocess: arguments go in as JSON on stdin and the result comes back as
//! JSON on a private pipe, so the envelope, the conversion rules and the error
//! reports are the same - only the data crosses by value (plain dicts/lists
//! in, no shared handles; bytes as ndata's hex string).

use std::path::PathBuf;

use ndata::data::Data;
use ndata::dataarray::DataArray;
use ndata::databytes::DataBytes;
use ndata::dataobject::DataObject;

use crate::code::CodeException;
use crate::DataStore;

#[derive(Debug)]
pub struct PyCmd {
  lib: String,
  id: String,
}

/// Everything needed to invoke one Python command.
#[derive(Debug, Clone)]
pub struct PyCall {
  /// `lib:ctl:cmd`, for messages.
  pub label: String,
  /// The directory on `sys.path` that holds the `lib/ctl/cmd.py` package tree.
  pub base_dir: PathBuf,
  /// Dotted module name, `lib.ctl.cmd`.
  pub module: String,
  /// The generated source file.
  pub file: PathBuf,
  /// The user's function within the module (the command name).
  pub func: String,
  /// Declared return type (`String`, `JSONObject`, ..., `FLAT`).
  pub returntype: String,
  /// The declared params only, all present.
  pub args: DataObject,
}

impl PyCmd {
  pub fn new(lib: &str, id: &str) -> PyCmd {
    PyCmd { lib: lib.to_string(), id: id.to_string() }
  }

  pub fn execute(&self, args: DataObject) -> Result<DataObject, CodeException> {
    let call = match PyCmd::prepare(&self.lib, &self.id, args) {
      Ok(c) => c,
      Err(msg) => return Ok(err_envelope(&msg)),
    };
    Ok(match run(&call) {
      Ok(envelope) => envelope,
      Err(msg) => {
        eprintln!("Python command {} failed: {}", call.label, msg);
        err_envelope(&msg)
      }
    })
  }

  /// The command's paths and names, with no arguments.
  pub fn locate(lib: &str, id: &str) -> Result<PyCall, String> {
    PyCmd::locate_impl(lib, id).map(|(call, _)| call)
  }

  fn locate_impl(lib: &str, id: &str) -> Result<(PyCall, DataObject), String> {
    let store = DataStore::new();
    let cmd_def = store.get_data(lib, id).get_object("data");
    if !cmd_def.has("python") {
      return Err(format!("{}:{} is not a Python command", lib, id));
    }
    let impl_def = store.get_data(lib, &cmd_def.get_string("python")).get_object("data");
    // Older records carry the control and command names on the impl record;
    // records written by newbound's dev.code do not: the name is on the
    // command record, the control is the one that lists the command.
    let func = if impl_def.has("cmd") { impl_def.get_string("cmd") } else { cmd_def.get_string("name") };
    let ctl = if impl_def.has("ctl") {
      impl_def.get_string("ctl")
    } else {
      control_of(&store, lib, id).ok_or_else(|| format!("no control in library {} lists command {}", lib, id))?
    };
    let base_dir = store.get_lib_root(lib).join("src");
    let file = base_dir.join(lib).join(&ctl).join(format!("{}.py", func));
    let call = PyCall {
      label: format!("{}:{}:{}", lib, ctl, func),
      base_dir,
      module: format!("{}.{}.{}", lib, ctl, func),
      file,
      func,
      returntype: impl_def.get_string("returntype"),
      args: DataObject::new(),
    };
    Ok((call, impl_def))
  }

  /// Resolves the command's source file and selects its declared params from
  /// `args`. A declared param missing from `args` is an error, worded exactly
  /// as the generated Rust wrapper words it.
  pub fn prepare(lib: &str, id: &str, args: DataObject) -> Result<PyCall, String> {
    let (mut call, impl_def) = PyCmd::locate_impl(lib, id)?;
    let mut call_args = DataObject::new();
    for p in impl_def.get_array("params").objects() {
      if let Data::DObject(_) = p {
        let name = p.object().get_string("name");
        match args.try_get_property(&name) {
          Ok(v) => call_args.set_property(&name, v),
          Err(_) => return Err(format!("missing required parameter: {}", name)),
        }
      }
    }
    call.args = call_args;
    Ok(call)
  }


  /// The generated source file. Panics if the command is not a Python
  /// command, as reading a missing record does.
  pub fn get_path(lib: &str, id: &str) -> String {
    match PyCmd::locate(lib, id) {
      Ok(call) => call.file.display().to_string(),
      Err(e) => panic!("{}", e),
    }
  }
}

/// The name of the control in `lib` that lists command `id`.
pub(crate) fn control_of(store: &DataStore, lib: &str, id: &str) -> Option<String> {
  let list = store.get_data(lib, "controls").get_object("data").get_array("list");
  for c in list.objects() {
    let c = c.object();
    let ctl = store.get_data(lib, &c.get_string("id")).get_object("data");
    if ctl.has("cmd") && ctl.get_array("cmd").objects().iter().any(|e| e.object().get_string("id") == id) {
      return Some(c.get_string("name"));
    }
  }
  None
}

/// Runs a prepared call in this build's mode: the embedded interpreter with
/// `python_runtime`, else a subprocess. Returns the success envelope
/// `{"a": value}` - an owning object, never a bare `Data` whose referent
/// nothing holds - or the error report.
pub fn run(call: &PyCall) -> Result<DataObject, String> {
  #[cfg(feature = "python_runtime")]
  return crate::pyenv::call(call);
  #[cfg(not(feature = "python_runtime"))]
  return subprocess::call(call);
}

/// An owned ndata value: a container is held as its handle, never as a bare
/// `Data` reference. Values cross the Python boundary only in this form, and
/// are stored only through ndata's handle-taking methods (`put_object`,
/// `push_array`, ...), which take over the handle's reference. A bare
/// `Data::DObject` holds no reference, so passing one around after its last
/// handle is dropped lets a concurrent ndata GC free it first.
pub enum Value {
  Object(DataObject),
  Array(DataArray),
  Bytes(DataBytes),
  /// String, Integer, Float, Boolean or Null - never a container.
  Scalar(Data),
}

impl Value {
  /// Takes a handle on `d`'s container through ndata's own accessors. `d`
  /// must come from an ndata getter on a container that is still alive.
  pub fn from_data(d: Data) -> Value {
    match d {
      Data::DObject(_) => Value::Object(d.object()),
      Data::DArray(_) => Value::Array(d.array()),
      Data::DBytes(_) => Value::Bytes(d.bytes()),
      scalar => Value::Scalar(scalar),
    }
  }

  /// Stores the value under `key`.
  pub fn put(self, target: &mut DataObject, key: &str) {
    match self {
      Value::Object(o) => target.put_object(key, o),
      Value::Array(a) => target.put_array(key, a),
      Value::Bytes(b) => target.put_bytes(key, b),
      Value::Scalar(d) => target.set_property(key, d),
    }
  }

  /// Appends the value.
  pub fn push(self, target: &mut DataArray) {
    match self {
      Value::Object(o) => target.push_object(o),
      Value::Array(a) => target.push_array(a),
      Value::Bytes(b) => target.push_bytes(b),
      Value::Scalar(d) => target.push_property(d),
    }
  }

  /// Replaces the value at `index`.
  pub fn put_at(self, target: &mut DataArray, index: usize) {
    match self {
      Value::Object(o) => target.put_object(index, o),
      Value::Array(a) => target.put_array(index, a),
      Value::Bytes(b) => target.put_bytes(index, b),
      Value::Scalar(d) => target.set_property(index, d),
    }
  }
}

/// A CPython bundled next to the executable, in `<exe dir>/python` - as
/// release bundles ship a relocatable python-build-standalone build. The
/// embedded runtime sets PYTHONHOME to it (a relocatable build needs that
/// when embedded), and subprocesses prefer it over the system python3.
pub fn bundled_python_home() -> Option<PathBuf> {
  let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
  let home = exe.parent()?.join("python");
  if home.join("lib").is_dir() || home.join("Lib").is_dir() { Some(home) } else { None }
}

static STDOUT_IS_PROTOCOL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Declares that this process's stdout carries a protocol - the MCP server
/// does (JSON-RPC, one message per line) - so a Python command's stdout
/// (print) must not reach it: it goes to stderr, still labelled. Call it
/// before any Python command runs (the interpreter's streams are set up
/// once, at its first call).
pub fn stdout_is_protocol() {
  STDOUT_IS_PROTOCOL.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// The fd Python commands' stdout is written to: 1, or 2 under a protocol.
pub fn python_stdout_fd() -> i32 {
  if STDOUT_IS_PROTOCOL.load(std::sync::atomic::Ordering::SeqCst) { 2 } else { 1 }
}

/// The Python interpreters to try for a subprocess, in order: the venv next
/// to `data/` (as the embedded runtime uses), then `python3`, then `python`.
fn python_interpreters() -> Vec<String> {
  let mut v = Vec::new();
  if let Ok(root) = DataStore::new().root.canonicalize() {
    if let Some(parent) = root.parent() {
      let venv = parent.join("venv");
      for p in [venv.join("bin").join("python"), venv.join("Scripts").join("python.exe")] {
        if p.exists() { v.push(p.display().to_string()); }
      }
    }
  }
  if let Some(home) = bundled_python_home() {
    for p in [home.join("bin").join("python3"), home.join("python.exe")] {
      if p.exists() { v.push(p.display().to_string()); }
    }
  }
  v.push("python3".to_string());
  v.push("python".to_string());
  v
}

/// The checker run by `check`, after PY_SUPPORT. argv: path func base module
/// ("mypy" | "skip").
const PY_CHECKER: &str = r#"
def _check_main():
    import json
    path, func, base, module, type_check = _sys.argv[1:6]
    result_fd = _os.dup(1)
    _os.dup2(2, 1)   # nothing the module prints may corrupt the result
    execute_line, body_def = _layout(path, func)
    problems = []

    def add(tool, severity, lineno, message):
        where = _where(lineno, execute_line, body_def)
        if where is None:
            where = "generated line %d" % lineno if lineno else "module"
        problems.append({"tool": tool, "severity": severity, "line": lineno or 0,
                         "where": where, "message": message})

    with open(path, encoding="utf-8") as f:
        source = f.read()
    try:
        compile(source, path, "exec")
        compiled = True
    except SyntaxError as e:
        add("python", "error", e.lineno, "%s: %s" % (type(e).__name__, e.msg))
        compiled = False

    if compiled:
        # Import it as the runtime would: missing packages, errors in the
        # imports or module-level code. The server-only modules are stand-ins.
        _install_flowlang_stub()
        _install_ndata_placeholder()
        _sys.path[:] = [p for p in _sys.path if p not in ("", ".")]
        if base not in _sys.path:
            _sys.path.append(base)
        try:
            mod = _import_new(module, base)
            if not callable(getattr(mod, func, None)):
                add("import", "error", None, "the module defines no function %r" % func)
        except BaseException as e:
            lineno = None
            for frame in _traceback.extract_tb(e.__traceback__):
                if _same_file(frame.filename, path):
                    lineno = frame.lineno
            if isinstance(e, SyntaxError) and e.filename and _same_file(e.filename, path):
                lineno = e.lineno
            add("import", "error", lineno, _traceback.format_exception_only(type(e), e)[-1].strip())

    mypy = "not installed" if type_check == "mypy" else "skipped"
    if compiled and type_check == "mypy":
        try:
            from mypy import api as mypy_api
        except ImportError:
            mypy_api = None
        if mypy_api is not None:
            _os.environ["MYPYPATH"] = base
            out, err, code = mypy_api.run([
                path, "--namespace-packages", "--explicit-package-bases", "--ignore-missing-imports",
                "--follow-imports", "silent", "--no-error-summary", "--show-error-codes", "--no-pretty",
                "--cache-dir", _os.devnull,
            ])
            mypy = "ran"
            for line in out.splitlines():
                m = _re.match(r"^(.*?):(\d+): (error|warning|note): (.*)$", line)
                if m and _same_file(m.group(1), path):
                    add("mypy", m.group(3), int(m.group(2)), m.group(4))

    out = json.dumps({"mypy": mypy, "python": _sys.version.split()[0], "problems": problems})
    with _os.fdopen(result_fd, "w", encoding="utf-8") as f:
        f.write(out)

_check_main()
"#;

/// Checks a Python command's generated source without running the command,
/// in a fresh interpreter process (the venv's, if any): it compiles, imports
/// as the runtime would (missing packages, errors in imports or module-level
/// code), and - when mypy is installed in that Python - type-checks against
/// the ndata_py/flowlang stubs. Problems in the command's own file are
/// reported where its author wrote them ("body line N", "imports line N"):
///
///   {"status": "ok", "command": "lib:ctl:cmd", "file": ..., "python": "3.13.1",
///    "mypy": "ran" | "not installed" | "skipped", "clean": bool,
///    "problems": [{"tool": "python"|"import"|"mypy", "severity": "error"|"note"|...,
///                  "where": "body line 3", "line": 12, "message": ...}]}
///
/// `clean` is false when any problem has severity "error". `status` is
/// "err" only when the check itself cannot run.
pub fn check(lib: &str, id: &str) -> DataObject {
  check_with(lib, id, true)
}

/// `check` without mypy: compile and import only - fast enough to run on
/// every save (newbound's dev.compile does). `mypy` is "skipped".
pub fn check_quick(lib: &str, id: &str) -> DataObject {
  check_with(lib, id, false)
}

fn check_with(lib: &str, id: &str, type_check: bool) -> DataObject {
  let mut err = DataObject::new();
  err.put_string("status", "err");
  let call = match std::panic::catch_unwind(|| PyCmd::locate(lib, id)) {
    Ok(Ok(c)) => c,
    Ok(Err(e)) => { err.put_string("msg", &e); return err; }
    Err(_) => { err.put_string("msg", &format!("no command {} in library {}", id, lib)); return err; }
  };
  if !call.file.exists() {
    err.put_string("msg", &format!("{}: the generated source {} does not exist - build the command first", call.label, call.file.display()));
    return err;
  }
  // The checker runs in the package root: hand it absolute paths.
  let mut call = call;
  call.file = call.file.canonicalize().unwrap_or(call.file);
  call.base_dir = call.base_dir.canonicalize().unwrap_or(call.base_dir);
  let script = format!("{}\n{}", PY_SUPPORT, PY_CHECKER);
  for exe in python_interpreters() {
    let out = std::process::Command::new(&exe)
      .arg("-c").arg(&script)
      .arg(call.file.display().to_string()).arg(&call.func)
      .arg(call.base_dir.display().to_string()).arg(&call.module)
      .arg(if type_check { "mypy" } else { "skip" })
      .current_dir(&call.base_dir)
      .output();
    let out = match out {
      Ok(o) => o,
      Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
      Err(e) => { err.put_string("msg", &format!("could not start {}: {}", exe, e)); return err; }
    };
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let reply = match DataObject::try_from_string(&text) {
      Ok(r) => r,
      Err(_) => {
        err.put_string("msg", &format!("{} exited with {} without a result: {}", exe, out.status, String::from_utf8_lossy(&out.stderr)));
        return err;
      }
    };
    let problems = reply.get_array("problems");
    let clean = !problems.objects().iter().any(|p| p.object().get_string("severity") == "error");
    let mut o = DataObject::new();
    o.put_string("status", "ok");
    o.put_string("command", &call.label);
    o.put_string("file", &call.file.display().to_string());
    o.put_string("interpreter", &exe);
    o.put_string("python", &reply.get_string("python"));
    o.put_string("mypy", &reply.get_string("mypy"));
    o.put_boolean("clean", clean);
    o.put_array("problems", problems);
    return o;
  }
  err.put_string("msg", "no Python interpreter found (python3 or python)");
  err
}

/// `{"a": value}` - what a generated Rust wrapper returns on success.
pub fn ok_envelope(value: Value) -> DataObject {
  let mut o = DataObject::new();
  value.put(&mut o, "a");
  o
}

/// `{"a": {"status": "err", "msg": msg}}` - what a generated Rust wrapper
/// returns when the command panics.
pub fn err_envelope(msg: &str) -> DataObject {
  let mut e = DataObject::new();
  e.put_string("status", "err");
  e.put_string("msg", msg);
  let mut o = DataObject::new();
  o.put_object("a", e);
  o
}

/// Python helpers shared by both modes: the error report and (for the
/// subprocess) the JSON conversion that mirrors `pywrapper::py_any_to_data`.
pub const PY_SUPPORT: &str = r#"
import os as _os
import re as _re
import sys as _sys
import threading as _threading
import traceback as _traceback


def _same_file(a, b):
    try:
        return _os.path.realpath(a) == _os.path.realpath(b)
    except Exception:
        return False


def _layout(path, func):
    # A generated command file is: the imports, 'def execute(args):', then
    # 'def <func>(<params>):' followed by the user's body, indented.
    execute_line = body_def = None
    try:
        with open(path, encoding="utf-8") as f:
            for n, line in enumerate(f, 1):
                if execute_line is None and line.startswith("def execute("):
                    execute_line = n
                elif line.startswith("def " + func + "("):
                    body_def = n
                    break
    except OSError:
        pass
    return execute_line, body_def


def _where(lineno, execute_line, body_def):
    # A line of a generated command file in the terms its author wrote it.
    if lineno is None:
        return None
    if body_def is not None and lineno > body_def:
        return "body line %d" % (lineno - body_def)
    if execute_line is not None and lineno < execute_line:
        return "imports line %d" % lineno
    return None


def format_error(e, path, func, label):
    execute_line, body_def = _layout(path, func)

    def where(lineno):
        return _where(lineno, execute_line, body_def)

    loc = None
    if isinstance(e, SyntaxError) and e.filename and _same_file(e.filename, path):
        loc = where(e.lineno)
        head = "%s: %s" % (type(e).__name__, e.msg)
    else:
        for frame in _traceback.extract_tb(e.__traceback__):
            if _same_file(frame.filename, path):
                loc = where(frame.lineno) or loc
        head = _traceback.format_exception_only(type(e), e)[-1].strip()

    def annotate(m):
        if _same_file(m.group(1), path):
            w = where(int(m.group(2)))
            if w:
                return m.group(0) + " [" + w + "]"
        return m.group(0)

    tb = "".join(_traceback.format_exception(type(e), e, e.__traceback__))
    tb = _re.sub(r'File "([^"]+)", line (\d+)', annotate, tb)
    where_txt = label + (", " + loc if loc else "")
    return "%s (%s)\n\n%s" % (head, where_txt, tb.rstrip())


# Hot reload: per module, the ((mtime_ns, size), sha1) of its file as loaded.
_loaded = {}
_reload_lock = _threading.Lock()


def _stamp(path):
    try:
        st = _os.stat(path)
        return (st.st_mtime_ns, st.st_size)
    except OSError:
        return None


def _digest(path):
    import hashlib
    try:
        with open(path, "rb") as f:
            return hashlib.sha1(f.read()).hexdigest()
    except OSError:
        return None


def _unchanged(module, path):
    rec = _loaded.get(module)
    if rec is None:
        return False
    stamp = _stamp(path)
    if stamp != rec[0]:
        return False
    # A rewrite within the filesystem's timestamp granularity can keep both
    # mtime and size: for a recently modified file, compare the contents.
    import time
    if stamp is not None and time.time_ns() - stamp[0] < 2_000_000_000:
        return _digest(path) == rec[1]
    return True


def import_command(module, base, path):
    # Imports lib.ctl.cmd from the package root `base`, which the caller has
    # put at the END of sys.path.
    #
    # Hot reload: when the command's file (`path`) has changed since it was
    # loaded - flowb / newbound's dev.compile regenerated it - the module is
    # reloaded in place before the call. Cached bytecode is removed before
    # every load: Python validates a .pyc by whole-second mtime and size, so a
    # same-size rewrite within a second would otherwise run the old code. If
    # the new source fails (e.g. a SyntaxError) the call reports it, and the
    # next call tries again; the old code is never run silently.
    import importlib
    import importlib.util
    mod = _sys.modules.get(module)
    if mod is not None and _unchanged(module, path):
        return mod
    with _reload_lock:
        mod = _sys.modules.get(module)
        if mod is not None and _unchanged(module, path):
            return mod                      # another thread reloaded it
        record = (_stamp(path), _digest(path))
        # Before every load - a first import too: a .pyc left by an earlier
        # process can be stale in the same way (the subprocess mode imports
        # afresh on every call).
        try:
            _os.remove(importlib.util.cache_from_source(path))
        except OSError:
            pass
        _forget_listings(base, path)
        if mod is not None:
            mod = importlib.reload(mod)
        else:
            mod = _import_new(module, base)
        _loaded[module] = record
        return mod


def _forget_listings(base, path):
    # A new command file or directory created within the filesystem's
    # timestamp granularity may be missing from a finder's cached directory
    # listing. Only the command's own directories are refreshed:
    # importlib.invalidate_caches() also resets the metadata finders, which
    # costs ~45 ms in a fresh interpreter (every subprocess-mode call).
    d = _os.path.dirname(path)
    while True:
        finder = _sys.path_importer_cache.get(d)
        if finder is not None and hasattr(finder, "invalidate_caches"):
            finder.invalidate_caches()
        if d == base or len(d) <= len(base):
            break
        d = _os.path.dirname(d)


def _import_new(module, base):
    # If the library's name is already taken by another module (stdlib or
    # installed), say so instead of a bare "No module named 'email.ctl'".
    import importlib
    try:
        return importlib.import_module(module)
    except ImportError as e:
        top = module.split(".")[0]
        taken = _sys.modules.get(top)
        origin = None
        if taken is not None:
            origin = getattr(taken, "__file__", None) or next(iter(getattr(taken, "__path__", None) or []), None)
        root = _os.path.realpath(base) + _os.sep
        if taken is not None and not (origin and _os.path.realpath(origin).startswith(root)):
            where = ("the Python module at %s" % origin) if origin else ("the built-in module %r" % top)
            raise ImportError(
                "library %r cannot be imported as a Python package: that name is taken by "
                "%s; rename the library or the conflicting module" % (top, where)
            ) from e
        raise


# ---- flowlang.api: every command as an attribute path -------------------------
#
#   flowlang.api.testflow.testflow.test_add(300, 120)
#     == flowlang.call("testflow", "testflow", "test_add", 300, 120)
#
# Names resolve as call() resolves them (exact, or with `-` read as `_`).
# flowb's flowlang.pyi types this tree from the libraries in data/.

class _ApiNode:
    __slots__ = ("_call", "_path")

    def __init__(self, call, path):
        self._call = call
        self._path = path

    def __getattr__(self, name):
        if name.startswith("__"):
            raise AttributeError(name)
        return _ApiNode(self._call, self._path + (name,))

    def __call__(self, *args, **kwargs):
        if len(self._path) != 3:
            raise TypeError("flowlang.api.%s is not a command: use flowlang.api.<lib>.<ctl>.<cmd>(...)"
                            % ".".join(self._path))
        return self._call(*self._path, *args, **kwargs)

    def __repr__(self):
        return "<flowlang.api%s>" % "".join("." + p for p in self._path)


def make_api(call):
    return _ApiNode(call, ())


def install_api():
    _sys.modules["flowlang"].api = make_api(_sys.modules["flowlang"].call)


def _install_flowlang_stub():
    # flowlang.call needs the server process; a subprocess cannot call back
    # into it. Same names as the embedded module, with a clear error.
    import types
    m = types.ModuleType("flowlang")

    class CommandError(Exception):
        pass

    def call(lib, ctl, cmd, /, *args, **kwargs):
        raise RuntimeError(
            "flowlang.call(%r, %r, %r) needs the embedded Python runtime; "
            "build flowlang with --features=python_runtime" % (lib, ctl, cmd)
        )

    m.CommandError = CommandError
    m.call = call
    m.api = make_api(call)
    _sys.modules["flowlang"] = m


def _install_ndata_placeholder():
    # For the checker's import step only: ndata_py exists in the embedded
    # runtime alone. Importing names from it must work; using them need not.
    import types
    m = types.ModuleType("ndata_py")

    class NDataPyError(Exception):
        pass

    def unavailable(*a, **k):
        raise RuntimeError("ndata_py is only available in the embedded runtime")

    for name in ("NDataObject", "NDataArray", "NDataBytes"):
        setattr(m, name, type(name, (), {"__init__": unavailable}))
    m.NDataPyError = NDataPyError
    m.init = m.gc = m.to_py = unavailable
    _sys.modules.setdefault("ndata_py", m)


# ---- output: print(), sys.stderr and logging ----------------------------------
#
# sys.stdout / sys.stderr are replaced by line writers: each complete line is
# written straight to the process's file descriptor (one os.write, so it
# cannot interleave with another writer's line, and nothing waits in a
# block buffer when stdout is not a terminal), prefixed with the command that
# wrote it - "[py lib:ctl:cmd] ...". Threads a command starts do not inherit
# its label and write "[py] ...".

import contextvars as _contextvars
import io as _io

_label = _contextvars.ContextVar("flowlang_command_label", default=None)
# The prefix for output written outside any command ("" under flow pytest).
_UNLABELLED = "[py] "


class _CommandOutput(_io.TextIOBase):
    def __init__(self, fd, name):
        self._fd = fd
        self._name = name
        self._lock = _threading.Lock()
        self._partial = {}   # thread ident -> text not yet written
        self._open = set()   # threads whose current line is written in part

    @property
    def name(self):
        return self._name

    @property
    def encoding(self):
        return "utf-8"

    @property
    def errors(self):
        return "backslashreplace"

    def writable(self):
        return True

    def isatty(self):
        return False

    def fileno(self):
        return self._fd

    def _prefix(self):
        label = _label.get()
        return "[py %s] " % label if label else _UNLABELLED

    def write(self, s):
        if not isinstance(s, str):
            raise TypeError("write() argument must be str, not %s" % type(s).__name__)
        tid = _threading.get_ident()
        with self._lock:
            *lines, rest = (self._partial.pop(tid, "") + s).split("\n")
            if rest:
                self._partial[tid] = rest
            out = []
            for line in lines:
                out.append(("" if tid in self._open else self._prefix()) + line + "\n")
                self._open.discard(tid)
            self._emit("".join(out))
        return len(s)

    def flush(self):
        # Make the current line's text visible now, without ending the line
        # (progress output, print(..., end="", flush=True)).
        tid = _threading.get_ident()
        with self._lock:
            rest = self._partial.pop(tid, "")
            if rest:
                self._emit(("" if tid in self._open else self._prefix()) + rest)
                self._open.add(tid)

    def end_line(self):
        # At the end of a call: a line the command left unfinished is ended,
        # so it neither joins nor is attributed to the next writer's text.
        self.flush()
        tid = _threading.get_ident()
        with self._lock:
            if tid in self._open:
                self._open.discard(tid)
                self._emit("\n")

    def _emit(self, text):
        data = text.encode("utf-8", "backslashreplace")
        while data:
            data = data[_os.write(self._fd, data):]


def install_output(out_fd, err_fd):
    # Once per interpreter. The root logger gets a stderr handler unless one
    # is configured already; FLOWLANG_PYTHON_LOG_LEVEL sets its level
    # (default: Python's WARNING).
    import logging
    _sys.stdout = _CommandOutput(out_fd, "<stdout>")
    _sys.stderr = _CommandOutput(err_fd, "<stderr>")
    root = logging.getLogger()
    if not root.handlers:
        handler = logging.StreamHandler(_sys.stderr)
        handler.setFormatter(logging.Formatter("%(levelname)s %(name)s: %(message)s"))
        root.addHandler(handler)
    level = _os.environ.get("FLOWLANG_PYTHON_LOG_LEVEL")
    if level:
        root.setLevel(level.strip().upper())


def begin_call(label):
    return _label.set(label)


def end_call(token):
    # A partial line the command left belongs to it, not to the next writer.
    for stream in (_sys.stdout, _sys.stderr):
        if isinstance(stream, _CommandOutput):
            stream.end_line()
    _label.reset(token)


def _hint(type_name):
    if type_name in ("set", "frozenset"):
        return " - convert it explicitly, e.g. sorted(x) or list(x)"
    if type_name == "Decimal":
        return " - convert it explicitly with str(x) (exact) or float(x) (rounded)"
    if type_name in ("datetime", "date", "time"):
        return " - convert it explicitly, e.g. x.isoformat()"
    return " - convert it to str, int, float, bool, None, bytes, dict, list or tuple"


def to_json(v):
    # The same rules as the embedded runtime's py_any_to_data: exact or an
    # error. Bytes become ndata's JSON form for DBytes (hex pairs).
    import numbers
    import operator
    if v is None or isinstance(v, (bool, str)):
        return v
    if isinstance(v, int) or (isinstance(v, numbers.Integral) and not isinstance(v, float)):
        i = operator.index(v)
        if not (-(2 ** 63) <= i < 2 ** 63):
            raise OverflowError("int %r does not fit in a 64-bit signed integer (ndata Integer); "
                                "convert it explicitly, e.g. str(n)" % (i,))
        return i
    if isinstance(v, float) or isinstance(v, numbers.Real):
        return float(v)
    if isinstance(v, (bytes, bytearray)):
        return " ".join("%02X" % b for b in v)
    if isinstance(v, dict):
        out = {}
        for k, x in v.items():
            if not isinstance(k, str):
                raise TypeError("dict keys must be str to convert to ndata, got %s key %r"
                                % (type(k).__name__, k))
            out[k] = to_json(x)
        return out
    if isinstance(v, (list, tuple)):
        return [to_json(x) for x in v]
    raise TypeError("cannot convert Python %s to ndata%s" % (type(v).__name__, _hint(type(v).__name__)))
"#;

#[cfg(not(feature = "python_runtime"))]
mod subprocess {
  use super::{PyCall, PY_SUPPORT};
  use ndata::dataobject::DataObject;
  use std::io::Write;
  use std::process::{Command, Stdio};

  // argv: base_dir module func returntype file label; JSON args on stdin.
  // The result goes to a private dup of the original stdout; fd 1 itself is
  // pointed at stderr, so print() (even from C extensions) cannot corrupt it.
  const PY_RUNNER: &str = r#"
def _main():
    import json
    _install_flowlang_stub()
    base, module, func, rtype, path, label = _sys.argv[1:7]
    result_fd = _os.dup(1)
    _os.dup2(2, 1)
    # Both streams go to stderr here: the process's stdout carries the result.
    install_output(2, 2)
    token = begin_call(label)
    try:
        args = json.load(_sys.stdin)
        # As in the embedded runtime: no current-directory entry ('' from
        # -c), and the package root at the end of sys.path.
        _sys.path[:] = [p for p in _sys.path if p not in ("", ".")]
        if base not in _sys.path:
            _sys.path.append(base)
        result = getattr(import_command(module, base, path), func)(**args)
        if rtype == "FLAT" and not isinstance(result, dict):
            raise TypeError("a FLAT command must return a dict or NDataObject, got %s" % type(result).__name__)
        out = json.dumps({"ok": True, "value": to_json(result)}, allow_nan=False)
    except BaseException as e:
        out = json.dumps({"ok": False, "msg": format_error(e, path, func, label)})
    end_call(token)
    with _os.fdopen(result_fd, "w", encoding="utf-8") as f:
        f.write(out)

_main()
"#;

  pub(super) fn call(call: &PyCall) -> Result<DataObject, String> {
    let script = format!("{}\n{}", PY_SUPPORT, PY_RUNNER);
    let mut last_err = String::new();
    for exe in super::python_interpreters() {
      let child = Command::new(&exe)
        .arg("-c").arg(&script)
        .arg(call.base_dir.display().to_string())
        .arg(&call.module)
        .arg(&call.func)
        .arg(&call.returntype)
        .arg(call.file.display().to_string())
        .arg(&call.label)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn();
      let mut child = match child {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => { last_err = format!("{}: {}", exe, e); continue; }
        Err(e) => return Err(format!("could not start {}: {}", exe, e)),
      };
      if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(call.args.to_string().as_bytes());
      }
      let output = child.wait_with_output().map_err(|e| format!("{} failed: {}", exe, e))?;
      let text = String::from_utf8_lossy(&output.stdout).to_string();
      let reply = DataObject::try_from_string(&text).map_err(|_| {
        format!("{} exited with {} without a result for {}", exe, output.status, call.label)
      })?;
      if reply.has("ok") && reply.get_boolean("ok") {
        return Ok(super::ok_envelope(super::Value::from_data(reply.get_property("value"))));
      }
      return Err(reply.get_string("msg"));
    }
    Err(format!(
      "no Python interpreter found to run {} ({}); install python3, or build with --features=python_runtime",
      call.label, last_err
    ))
  }
}
