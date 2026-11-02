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

  /// Resolves the command's source file and selects its declared params from
  /// `args`. A declared param missing from `args` is an error, worded exactly
  /// as the generated Rust wrapper words it.
  pub fn prepare(lib: &str, id: &str, args: DataObject) -> Result<PyCall, String> {
    let store = DataStore::new();
    let cmd_def = store.get_data(lib, id).get_object("data");
    let impl_id = cmd_def.get_string("python");
    let impl_def = store.get_data(lib, &impl_id).get_object("data");
    let ctl = impl_def.get_string("ctl");
    let func = impl_def.get_string("cmd");
    let base_dir = store.get_lib_root(lib).join("src");
    let file = base_dir.join(lib).join(&ctl).join(format!("{}.py", func));

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

    Ok(PyCall {
      label: format!("{}:{}:{}", lib, ctl, func),
      base_dir,
      module: format!("{}.{}.{}", lib, ctl, func),
      file,
      func,
      returntype: impl_def.get_string("returntype"),
      args: call_args,
    })
  }

  pub fn get_path(lib: &str, id: &str) -> String {
    let store = DataStore::new();
    let impl_id = store.get_data(lib, id).get_object("data").get_string("python");
    let impl_def = store.get_data(lib, &impl_id).get_object("data");
    let ctl = impl_def.get_string("ctl");
    let cmd = impl_def.get_string("cmd");
    let root = store.get_lib_root(lib);
    root.join("src").join(lib).join(&ctl).join(cmd + ".py").display().to_string()
  }
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


def format_error(e, path, func, label):
    execute_line, body_def = _layout(path, func)

    def where(lineno):
        if lineno is None:
            return None
        if body_def is not None and lineno > body_def:
            return "body line %d" % (lineno - body_def)
        if execute_line is not None and lineno < execute_line:
            return "imports line %d" % lineno
        return None

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


def import_command(module, base):
    # Imports lib.ctl.cmd from the package root `base`, which the caller has
    # put at the END of sys.path. If the library's name is already taken by
    # another module (stdlib or installed), say so instead of a bare
    # "No module named 'email.ctl'".
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
  use crate::DataStore;
  use ndata::dataobject::DataObject;
  use std::io::Write;
  use std::process::{Command, Stdio};

  // argv: base_dir module func returntype file label; JSON args on stdin.
  // The result goes to a private dup of the original stdout; fd 1 itself is
  // pointed at stderr, so print() (even from C extensions) cannot corrupt it.
  const PY_RUNNER: &str = r#"
def _install_flowlang_stub():
    # flowlang.call needs the server process; a subprocess cannot call back
    # into it. Same names as the embedded module, with a clear error.
    import types
    m = types.ModuleType("flowlang")

    class CommandError(Exception):
        pass

    def call(lib, ctl, cmd, /, **kwargs):
        raise RuntimeError(
            "flowlang.call(%r, %r, %r) needs the embedded Python runtime; "
            "build flowlang with --features=python_runtime" % (lib, ctl, cmd)
        )

    m.CommandError = CommandError
    m.call = call
    _sys.modules["flowlang"] = m


def _main():
    import json
    _install_flowlang_stub()
    base, module, func, rtype, path, label = _sys.argv[1:7]
    result_fd = _os.dup(1)
    _os.dup2(2, 1)
    try:
        args = json.load(_sys.stdin)
        # As in the embedded runtime: no current-directory entry ('' from
        # -c), and the package root at the end of sys.path.
        _sys.path[:] = [p for p in _sys.path if p not in ("", ".")]
        if base not in _sys.path:
            _sys.path.append(base)
        result = getattr(import_command(module, base), func)(**args)
        if rtype == "FLAT" and not isinstance(result, dict):
            raise TypeError("a FLAT command must return a dict or NDataObject, got %s" % type(result).__name__)
        out = json.dumps({"ok": True, "value": to_json(result)}, allow_nan=False)
    except BaseException as e:
        out = json.dumps({"ok": False, "msg": format_error(e, path, func, label)})
    _sys.stdout.flush()
    with _os.fdopen(result_fd, "w", encoding="utf-8") as f:
        f.write(out)

_main()
"#;

  fn interpreters() -> Vec<String> {
    let mut v = Vec::new();
    // A venv next to data/ is used, as in the embedded runtime.
    if let Ok(root) = DataStore::new().root.canonicalize() {
      if let Some(parent) = root.parent() {
        let venv = parent.join("venv");
        for p in [venv.join("bin").join("python"), venv.join("Scripts").join("python.exe")] {
          if p.exists() { v.push(p.display().to_string()); }
        }
      }
    }
    v.push("python3".to_string());
    v.push("python".to_string());
    v
  }

  pub(super) fn call(call: &PyCall) -> Result<DataObject, String> {
    let script = format!("{}\n{}", PY_SUPPORT, PY_RUNNER);
    let mut last_err = String::new();
    for exe in interpreters() {
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
