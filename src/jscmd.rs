//! JavaScript commands: the same contract as Rust and Python commands.
//!
//! A JavaScript command returns exactly what a Rust command returns, so
//! callers (flows, newbound's `format_result`, MCP tools) never special-case
//! the language:
//!
//! - success: `{"a": <return value>}`
//! - failure: `{"a": {"status": "err", "msg": <report>}}`, where the report
//!   is the exception, its location in the command's own body
//!   ("body line N"), and the stack.
//!
//! The command's body is the body of a function whose parameters are the
//! declared params, in declared order. Arguments go in by value (JSON
//! semantics; bytes as ndata's hex string) and the return value comes back
//! the same way: objects, arrays, strings, numbers, booleans and null
//! convert; `undefined` is null. A function, symbol, BigInt, Map or Promise
//! is an error that names it. A `FLAT` command must return an object.
//!
//! One V8 isolate runs every JavaScript command, on a thread of its own: an
//! isolate belongs to the thread that made it, so callers on any thread hand
//! their call to that thread and wait for its answer. Global state a command
//! leaves behind persists between calls, as with Python. Commands run one at
//! a time. A panic inside the engine restarts it with fresh state (and is
//! reported as that call's error) instead of taking every later call down.
//!
//! A command record names its language `"js"` or `"javascript"` (newbound's
//! `dev.code` writes the latter); both are the same language, and the body is
//! read from whichever key the implementation record carries.
//!
//! `console.log`/`info`/`debug` write to stdout (stderr when stdout carries a
//! protocol, as under `flowmcp`), `console.warn`/`error` to stderr, as
//! complete lines labelled with the command: `[js lib:ctl:cmd] message`.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::mpsc;
use std::sync::Mutex;

use deno_core::serde_v8;
use deno_core::v8;
use deno_core::JsRuntime;
use deno_core::RuntimeOptions;
use ndata::data::Data;
use ndata::dataarray::DataArray;
use ndata::dataobject::DataObject;

use crate::code::CodeException;
use crate::pycmd::{err_envelope, ok_envelope, Value};
use crate::DataStore;

/// The language tags a command record may carry for JavaScript.
pub const LANGS: [&str; 2] = ["js", "javascript"];

/// True for a command `type` that names JavaScript.
pub fn is_js(lang: &str) -> bool {
  LANGS.contains(&lang)
}

#[derive(Debug)]
pub struct JSCmd {
  lib: String,
  id: String,
}

/// Everything needed to invoke one JavaScript command.
#[derive(Debug, Clone)]
pub struct JsCall {
  /// `lib:ctl:cmd`, for messages and console labels.
  pub label: String,
  /// Declared parameter names, in order. Each must be a JavaScript
  /// identifier.
  pub params: Vec<String>,
  /// The function body.
  pub body: String,
  /// Declared return type (`String`, `JSONObject`, ..., `FLAT`).
  pub returntype: String,
  /// The declared params only, all present.
  pub args: DataObject,
}

impl JSCmd {
  /// Starts the engine thread if it is not running. Calls start it on
  /// demand; this is only for warming it up.
  pub fn init() {
    let _ = engine();
  }

  pub fn new(lib: &str, id: &str) -> JSCmd {
    JSCmd { lib: lib.to_string(), id: id.to_string() }
  }

  pub fn execute(&self, args: DataObject) -> Result<DataObject, CodeException> {
    let call = match JSCmd::prepare(&self.lib, &self.id, args) {
      Ok(c) => c,
      Err(msg) => return Ok(err_envelope(&msg)),
    };
    Ok(match run(&call) {
      Ok(envelope) => envelope,
      Err(msg) => {
        eprintln!("JavaScript command {} failed: {}", call.label, msg);
        err_envelope(&msg)
      }
    })
  }

  /// True when the command's records exist and carry a body.
  pub fn exists(lib: &str, id: &str) -> bool {
    match JSCmd::locate(lib, id) {
      Ok((_, impl_def)) => body_key(&impl_def).is_some(),
      Err(_) => false,
    }
  }

  /// The command's label and implementation record.
  fn locate(lib: &str, id: &str) -> Result<(String, DataObject), String> {
    let store = DataStore::new();
    if !store.exists(lib, id) {
      return Err(format!("no command {} in library {}", id, lib));
    }
    let cmd_def = store.get_data(lib, id).get_object("data");
    let impl_id = impl_key(&cmd_def)
      .map(|k| cmd_def.get_string(k))
      .ok_or_else(|| format!("{}:{} is not a JavaScript command", lib, id))?;
    if !store.exists(lib, &impl_id) {
      return Err(format!("the implementation record {} of {}:{} is missing", impl_id, lib, id));
    }
    let impl_def = store.get_data(lib, &impl_id).get_object("data");
    // Older records carry the control and command names on the impl record;
    // records written by newbound's dev.code do not: the name is on the
    // command record, the control is the one that lists the command.
    let name = if impl_def.has("cmd") { impl_def.get_string("cmd") } else { cmd_def.get_string("name") };
    let ctl = if impl_def.has("ctl") {
      impl_def.get_string("ctl")
    } else {
      crate::pycmd::control_of(&store, lib, id)
        .ok_or_else(|| format!("no control in library {} lists command {}", lib, id))?
    };
    Ok((format!("{}:{}:{}", lib, ctl, name), impl_def))
  }

  /// Reads the command's body and selects its declared params from `args`.
  /// A declared param missing from `args` is an error, worded exactly as the
  /// generated Rust wrapper words it.
  pub fn prepare(lib: &str, id: &str, args: DataObject) -> Result<JsCall, String> {
    let (label, impl_def) = JSCmd::locate(lib, id)?;
    let key = body_key(&impl_def).ok_or_else(|| format!("{} has no JavaScript body", label))?;
    let body = impl_def.get_string(key);
    let mut params = Vec::new();
    let mut call_args = DataObject::new();
    if impl_def.has("params") {
      for p in impl_def.get_array("params").objects() {
        if let Data::DObject(_) = p {
          let name = p.object().get_string("name");
          match args.try_get_property(&name) {
            Ok(v) => call_args.set_property(&name, v),
            Err(_) => return Err(format!("missing required parameter: {}", name)),
          }
          params.push(name);
        }
      }
    }
    let returntype = if impl_def.has("returntype") { impl_def.get_string("returntype") } else { "Any".to_string() };
    Ok(JsCall { label, params, body, returntype, args: call_args })
  }
}

/// The key on a command record that names its implementation record.
fn impl_key(cmd_def: &DataObject) -> Option<&'static str> {
  if cmd_def.has("type") {
    let t = cmd_def.get_string("type");
    if let Some(k) = LANGS.iter().find(|k| **k == t) {
      if cmd_def.has(k) { return Some(k); }
    }
  }
  LANGS.iter().copied().find(|k| cmd_def.has(k))
}

/// The key on an implementation record that holds the body: the record's
/// own type first, then either JavaScript key.
fn body_key(impl_def: &DataObject) -> Option<&'static str> {
  impl_key(impl_def)
}

/// Runs a prepared call on the engine thread. Returns the success envelope
/// `{"a": value}`, or the error report.
pub fn run(call: &JsCall) -> Result<DataObject, String> {
  for p in &call.params {
    if !is_identifier(p) {
      return Err(format!("parameter name {:?} of {} is not a JavaScript identifier", p, call.label));
    }
  }
  let job = Job {
    label: call.label.clone(),
    params: call.params.clone(),
    body: call.body.clone(),
    flat: call.returntype == "FLAT",
    stdout_to_err: crate::pycmd::python_stdout_fd() == 2,
    args: call.params.iter().map(|p| {
      let mut o = DataObject::new();
      o.set_property("v", call.args.get_property(p));
      o.to_json().get("v").cloned().unwrap_or(serde_json::Value::Null)
    }).collect(),
  };
  let (reply_tx, reply_rx) = mpsc::channel();
  // The engine thread is gone only if it was killed from outside the panic
  // guard; start a new one once before giving up.
  let mut msg = Msg { job, reply: reply_tx };
  for attempt in 0..2 {
    match engine().send(msg) {
      Ok(()) => break,
      Err(mpsc::SendError(m)) => {
        msg = m;
        restart_engine();
        if attempt == 1 { return Err("the JavaScript engine is not running".to_string()); }
      }
    }
  }
  let value = reply_rx.recv().map_err(|_| format!("the JavaScript engine stopped while running {}", call.label))??;
  Ok(ok_envelope(json_to_value(value)))
}

/// What the engine thread is asked to run.
struct Job {
  label: String,
  params: Vec<String>,
  body: String,
  flat: bool,
  stdout_to_err: bool,
  args: Vec<serde_json::Value>,
}

struct Msg {
  job: Job,
  reply: mpsc::Sender<Result<serde_json::Value, String>>,
}

static ENGINE: Mutex<Option<mpsc::Sender<Msg>>> = Mutex::new(None);

fn engine() -> mpsc::Sender<Msg> {
  let mut guard = ENGINE.lock().unwrap_or_else(|e| e.into_inner());
  if guard.is_none() {
    *guard = Some(spawn_engine());
  }
  guard.as_ref().unwrap().clone()
}

fn restart_engine() {
  let mut guard = ENGINE.lock().unwrap_or_else(|e| e.into_inner());
  *guard = Some(spawn_engine());
}

fn spawn_engine() -> mpsc::Sender<Msg> {
  let (tx, rx) = mpsc::channel::<Msg>();
  std::thread::Builder::new()
    .name("flowlang-js".to_string())
    .spawn(move || {
      let mut engine: Option<Engine> = None;
      for msg in rx {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
          if engine.is_none() { engine = Some(Engine::new()?); }
          engine.as_mut().unwrap().call(&msg.job)
        }));
        let result = match result {
          Ok(r) => r,
          Err(e) => {
            // The isolate's state after a panic is unknown: start over.
            engine = None;
            let what = if let Some(s) = e.downcast_ref::<&str>() { s.to_string() }
              else if let Some(s) = e.downcast_ref::<String>() { s.clone() }
              else { "unknown panic".to_string() };
            Err(format!("the JavaScript engine panicked running {} and was restarted: {}", msg.job.label, what))
          }
        };
        let _ = msg.reply.send(result);
      }
    })
    .expect("could not start the JavaScript engine thread");
  tx
}

/// Installed once per isolate: the command table, the call trampoline and a
/// console that labels its lines and keeps stdout clean under a protocol.
const PRELUDE: &str = r#"
(() => {
  const print = Deno.core.print;
  const state = { label: "", stdoutToErr: false };
  const fmt = (a) => {
    if (typeof a === "string") return a;
    if (a instanceof Error) return a.stack || String(a);
    try { const s = JSON.stringify(a); return s === undefined ? String(a) : s; }
    catch (_) { return String(a); }
  };
  const write = (isErr, args) => {
    const text = args.map(fmt).join(" ");
    const prefix = state.label ? "[js " + state.label + "] " : "[js] ";
    const lines = text.split("\n").map((l) => prefix + l).join("\n") + "\n";
    print(lines, isErr || state.stdoutToErr);
  };
  const console = {
    log: (...a) => write(false, a),
    info: (...a) => write(false, a),
    debug: (...a) => write(false, a),
    warn: (...a) => write(true, a),
    error: (...a) => write(true, a),
    trace: (...a) => write(true, [...a, new Error("trace").stack]),
  };
  Object.defineProperty(globalThis, "console", { value: console, writable: true, configurable: true });
  const cmds = Object.create(null);
  Object.defineProperty(globalThis, "__flowlang", {
    value: Object.freeze({
      cmds,
      call(key, args, label, stdoutToErr) {
        state.label = label;
        state.stdoutToErr = stdoutToErr;
        try { return cmds[key](...args); }
        finally { state.label = ""; }
      },
    }),
  });
})();
"#;

struct Engine {
  runtime: JsRuntime,
  /// Command label -> hash of the source it was registered from.
  registered: HashMap<String, u64>,
}

impl Engine {
  fn new() -> Result<Engine, String> {
    let mut runtime = JsRuntime::new(RuntimeOptions::default());
    runtime.execute_script("<flowlang prelude>", PRELUDE).map_err(|e| e.to_string())?;
    Ok(Engine { runtime, registered: HashMap::new() })
  }

  fn call(&mut self, job: &Job) -> Result<serde_json::Value, String> {
    let key = serde_json::Value::String(job.label.clone()).to_string();
    let script_name = format!("{}.js", job.label.replace(':', "/"));
    // The body starts on line 1 of the registered source, so the source's
    // line numbers are the body's own.
    let name = job.label.rsplit(':').next().filter(|n| is_identifier(n)).unwrap_or("command");
    let source = format!(
      "__flowlang.cmds[{}] = function {}({}) {{ {}\n}};",
      key, name, job.params.join(", "), job.body
    );
    let body_lines = job.body.lines().count().max(1) as i64;
    let hash = { let mut h = DefaultHasher::new(); source.hash(&mut h); h.finish() };
    if self.registered.get(&job.label) != Some(&hash) {
      self.registered.remove(&job.label);
      self.runtime.execute_script(script_name.clone(), source)
        .map_err(|e| report(&e, &script_name, body_lines))?;
      self.registered.insert(job.label.clone(), hash);
    }

    let call = format!(
      "__flowlang.call({}, {}, {}, {})",
      key,
      serde_json::Value::Array(job.args.clone()),
      key,
      job.stdout_to_err
    );
    let result = self.runtime.execute_script("<flowlang call>", call)
      .map_err(|e| report(&e, &script_name, body_lines))?;

    deno_core::scope!(scope, &mut self.runtime);
    let local = v8::Local::new(scope, result);
    let kind = if local.is_promise() { Some("a Promise (async commands are not supported yet)") }
      else if local.is_function() { Some("a function") }
      else if local.is_symbol() { Some("a symbol") }
      else if local.is_big_int() { Some("a BigInt") }
      else if local.is_map() { Some("a Map") }
      else if local.is_set() { Some("a Set") }
      else { None };
    if let Some(kind) = kind {
      return Err(format!("return value of {} could not be converted: {}", job.label, kind));
    }
    let value: serde_json::Value = serde_v8::from_v8(scope, local)
      .map_err(|e| format!("return value of {} could not be converted: {}", job.label, e))?;
    if job.flat && !value.is_object() {
      return Err(format!("a FLAT command must return an object, got {}", json_kind(&value)));
    }
    Ok(value)
  }
}

/// An exception as the caller sees it: the message, where in the command's
/// own body it happened, and the stack, without the engine's own frames.
fn report(e: &deno_core::error::JsError, script_name: &str, body_lines: i64) -> String {
  let line = e.frames.iter()
    .find(|f| f.file_name.as_deref() == Some(script_name))
    .and_then(|f| f.line_number);
  let mut msg = e.exception_message.clone();
  match line {
    Some(n) if n > body_lines => msg += " (at the end of the body)",
    Some(n) => msg += &format!(" (body line {})", n),
    None => {}
  }
  for l in e.to_string().lines() {
    if l.trim_start().starts_with("at ") && !l.contains("<flowlang prelude>") && !l.contains("<flowlang call>") {
      msg += "\n";
      msg += l;
    }
  }
  msg
}

fn json_kind(v: &serde_json::Value) -> &'static str {
  match v {
    serde_json::Value::Null => "null",
    serde_json::Value::Bool(_) => "a boolean",
    serde_json::Value::Number(_) => "a number",
    serde_json::Value::String(_) => "a string",
    serde_json::Value::Array(_) => "an array",
    serde_json::Value::Object(_) => "an object",
  }
}

fn json_to_value(v: serde_json::Value) -> Value {
  match v {
    serde_json::Value::Object(_) => Value::Object(DataObject::from_json(v)),
    serde_json::Value::Array(_) => Value::Array(DataArray::from_json(v)),
    serde_json::Value::String(s) => Value::Scalar(Data::DString(s)),
    serde_json::Value::Bool(b) => Value::Scalar(Data::DBoolean(b)),
    serde_json::Value::Null => Value::Scalar(Data::DNull),
    serde_json::Value::Number(n) => match n.as_i64() {
      Some(i) => Value::Scalar(Data::DInt(i)),
      None => Value::Scalar(Data::DFloat(n.as_f64().unwrap_or(f64::NAN))),
    },
  }
}

/// An ASCII JavaScript identifier that is not a reserved word.
fn is_identifier(s: &str) -> bool {
  const RESERVED: [&str; 45] = [
    "await", "break", "case", "catch", "class", "const", "continue", "debugger", "default",
    "delete", "do", "else", "enum", "export", "extends", "false", "finally", "for", "function",
    "if", "implements", "import", "in", "instanceof", "interface", "let", "new", "null",
    "package", "private", "protected", "public", "return", "static", "super", "switch", "this",
    "throw", "true", "try", "typeof", "var", "void", "while", "with",
  ];
  let mut chars = s.chars();
  match chars.next() {
    Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '$' => {}
    _ => return false,
  }
  chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$') && !RESERVED.contains(&s) && s != "yield"
}
