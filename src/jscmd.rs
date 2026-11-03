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
//! declared params, in declared order. The body may `await`: a command that
//! returns a Promise (an `async` body, or a returned promise) answers with
//! what the promise resolves to, and a rejection is the command's error.
//! Objects and arrays are live ndata (`jsndata`): an argument is a handle on
//! the caller's own object, changes reach the caller, and returning a handle
//! returns the same object; plain values written or returned become new
//! ndata. Bytes cross as `Uint8Array`. A function, symbol, BigInt, Map, Set,
//! Promise or cyclic structure cannot be stored - a TypeError names it. A
//! `FLAT` command must return an object.
//!
//! One V8 isolate runs every JavaScript command, on a thread of its own: an
//! isolate belongs to the thread that made it, so callers on any thread hand
//! their call to that thread and wait for its answer. Global state a command
//! leaves behind persists between calls, as with Python. The engine works
//! like Node's event loop: a call's synchronous part runs to completion,
//! and a call that awaits leaves the engine free for the next call while it
//! waits - many awaiting calls make progress together, and each is
//! answered as its promise settles. Timers and promises a call leaves
//! behind keep running between calls. A promise that can never settle (the
//! event loop has nothing left that could settle it) is an error, not a
//! hang. A panic inside the engine restarts it with fresh state (and is
//! reported to every call it was serving) instead of taking every later
//! call down.
//!
//! Every call has a time limit - 30 s unless `FLOWLANG_JS_TIMEOUT_MS` says
//! otherwise (`0`: none), or `set_default_timeout` / `JsCall::timeout` do -
//! covering its synchronous part and its awaiting alike. A call past its
//! limit is answered as timed out; JavaScript still running at the limit is
//! stopped (`terminate_execution`), and the engine carries on. Stopping
//! JavaScript discards the microtasks V8 had queued behind it, which may
//! belong to other awaiting calls: one that can then never settle says its
//! work was lost to the stop. Background work (timer callbacks) is limited
//! to the default per turn of the event loop.
//!
//! Commands call other commands through the `flowlang` global, mirroring
//! Python's `flowlang` module: `flowlang.call(lib, ctl, cmd, {name: value})`
//! or `flowlang.api.<lib>.<ctl>.<cmd>(positional...)` (names with `-`
//! written with `_`) run any command - Rust, Python, flow or JavaScript -
//! and return its value. An unknown command throws `ReferenceError`,
//! arguments that do not fit throw `TypeError`, and a failed command throws
//! `flowlang.CommandError`. A JavaScript callee runs in the same isolate and
//! returns its value as is (a Promise, if it is async).
//!
//! The JavaScript globals are V8's plus `console`, `setTimeout`,
//! `setInterval`, `clearTimeout`, `clearInterval`, `queueMicrotask`,
//! `flowlang`, and the web platform APIs of `jsweb`: `fetch`, `Headers`,
//! `Request`, `Response`, `URL`, `URLSearchParams`, `TextEncoder`,
//! `TextDecoder`, `atob`, `btoa`, `crypto` (`getRandomValues`,
//! `randomUUID`), `structuredClone`, `performance`, `Event`, `EventTarget`,
//! `AbortController`, `AbortSignal` and `DOMException`. `console.log`/`info`/`debug` write to stdout (stderr when
//! stdout carries a protocol, as under `flowmcp`), `console.warn`/`error`/
//! `trace` to stderr, as complete lines labelled with the command:
//! `[js lib:ctl:cmd] message`.
//!
//! A command record names its language `"js"` or `"javascript"` (newbound's
//! `dev.code` writes the latter); both are the same language, and the body is
//! read from whichever key the implementation record carries. `"typescript"`
//! / `"ts"` commands run here too.
//!
//! A command with imports (its record's `import` field: ES `import`
//! statements, resolved from node_modules next to data/), and every
//! TypeScript command, runs from an esbuild bundle (`builder::javascript::
//! bundle`): the bundle is installed as a script under a name unique to its
//! version, and its source map - served by the engine's module loader - maps
//! stack traces back to the generated file, whose lines `Layout` turns into
//! "imports line N" / "body line N".

use std::cell::Cell;
use std::collections::hash_map::DefaultHasher;
use std::future::{poll_fn, Future};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Condvar, Mutex};
use std::task::Poll;
use std::time::{Duration, Instant};

use deno_core::v8;
use deno_core::{JsRuntime, PollEventLoopOptions, RuntimeOptions};
use ndata::data::Data;
use ndata::dataarray::DataArray;
use ndata::dataobject::DataObject;
use tokio::sync::mpsc;

use crate::callapi::{resolve_command, run_command, CallFailure};
use crate::code::CodeException;
use crate::pycmd::{err_envelope, ok_envelope, Value};
use crate::DataStore;

pub use crate::builder::javascript::{is_identifier, uses_await, Bundle, BundleError, JsSource, Layout};
use crate::builder::javascript as gen;

/// The language tags of the commands this engine runs: JavaScript and
/// TypeScript.
pub const LANGS: [&str; 4] = ["js", "javascript", "typescript", "ts"];

/// True for a command `type` this engine runs (JavaScript or TypeScript).
pub fn is_js(lang: &str) -> bool {
  LANGS.contains(&lang)
}

/// The time limit when nothing else sets one.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

static TIMEOUT_OVERRIDE: Mutex<Option<Option<Duration>>> = Mutex::new(None);

/// Sets the time limit for calls that do not carry their own (`None`: no
/// limit), overriding `FLOWLANG_JS_TIMEOUT_MS`.
pub fn set_default_timeout(timeout: Option<Duration>) {
  *TIMEOUT_OVERRIDE.lock().unwrap_or_else(|e| e.into_inner()) = Some(timeout);
}

/// The time limit for calls that do not carry their own.
pub fn default_timeout() -> Option<Duration> {
  if let Some(t) = *TIMEOUT_OVERRIDE.lock().unwrap_or_else(|e| e.into_inner()) {
    return t;
  }
  match std::env::var("FLOWLANG_JS_TIMEOUT_MS").ok().and_then(|v| v.trim().parse::<u64>().ok()) {
    Some(0) => None,
    Some(ms) => Some(Duration::from_millis(ms)),
    None => Some(DEFAULT_TIMEOUT),
  }
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
  /// The time limit for this call; `None` uses `default_timeout()`.
  pub timeout: Option<Duration>,
  /// For a command that imports modules or is TypeScript: its esbuild
  /// bundle and the generated file's layout (for reporting lines).
  pub module: Option<Module>,
}

/// A bundled command: what runs, and where its lines are.
#[derive(Debug, Clone)]
pub struct Module {
  pub bundle: Arc<Bundle>,
  pub layout: Layout,
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

  /// Reads the command's code and selects its declared params from `args`.
  /// A declared param missing from `args` is an error, worded exactly as the
  /// generated Rust wrapper words it. A command that imports modules or is
  /// TypeScript comes with its bundle - built now if its inputs changed.
  pub fn prepare(lib: &str, id: &str, args: DataObject) -> Result<JsCall, String> {
    let (label, _) = JSCmd::locate(lib, id)?;
    let store = DataStore::new();
    let src = JsSource::read(&store, lib, id).ok_or_else(|| format!("{} has no JavaScript body", label))?;
    let mut call_args = DataObject::new();
    for (name, _) in &src.params {
      match args.try_get_property(name) {
        Ok(v) => call_args.set_property(name, v),
        Err(_) => return Err(format!("missing required parameter: {}", name)),
      }
    }
    let module = if src.needs_bundle() {
      let mut parts = label.splitn(3, ':').skip(1);
      let (ctl, cmd) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
      let layout = gen::layout(&src);
      let bundle = gen::bundle(&store, &store.get_lib_root(lib).join("src"), lib, ctl, cmd, &src)
        .map_err(|e| format!("{}: {}", label, e.text(&layout)))?;
      Some(Module { bundle, layout })
    } else {
      None
    };
    Ok(JsCall {
      label,
      params: src.params.iter().map(|(n, _)| n.clone()).collect(),
      body: src.body,
      returntype: src.returntype,
      args: call_args,
      timeout: None,
      module,
    })
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

/// How a command is installed in the isolate. The same source serves a call
/// from Rust and a call from another JavaScript command, so whichever path
/// installs a body first, the other sees it as current.
#[derive(Debug, Clone)]
struct Registration {
  /// The label as a JavaScript string literal.
  key: String,
  /// Identifies (label, params, body, bundle), as a JavaScript string literal.
  hash: String,
  /// Installs the function as `__flowlang.cmds[label]` and records `hash`.
  source: String,
  /// The script's name: `flowlang:lib/ctl/cmd.js` (a URL: `evalContext`
  /// requires one), or for a bundle `flowlang:lib/ctl/cmd.bundle.<key>.js`
  /// - unique per bundle, as the engine caches source maps by name.
  script_name: String,
  /// The name the author's code has in stack traces: the script's own, or
  /// for a bundle the generated file's (`flowlang:lib/ctl/cmd.ts`), through
  /// the source map.
  source_name: String,
  layout: Layout,
  /// A bundle's source map, registered under `script_name`.
  map: Option<Arc<Bundle>>,
}

fn registration(call: &JsCall) -> Result<Registration, String> {
  for p in &call.params {
    if !is_identifier(p) {
      return Err(format!("parameter name {:?} of {} is not a JavaScript identifier", p, call.label));
    }
  }
  let key = serde_json::Value::String(call.label.clone()).to_string();
  let hash = {
    let mut h = DefaultHasher::new();
    (&call.label, &call.params, &call.body, call.module.as_ref().map(|m| &m.bundle.key)).hash(&mut h);
    serde_json::Value::String(format!("{:016x}", h.finish())).to_string()
  };
  let path = call.label.replace(':', "/");
  if let Some(m) = &call.module {
    // The bundle defines __flowlang_module in a scope of its own; its first
    // line is esbuild's banner comment, so the wrapper's opening shares it
    // and every other line keeps its number for the source map.
    let source = format!(
      "__flowlang.cmds[{key}] = (() => {{ {code}\n; return __flowlang_module.default; }})(); __flowlang.hashes[{key}] = {hash};",
      key = key, code = m.bundle.code, hash = hash
    );
    return Ok(Registration {
      key,
      hash,
      source,
      script_name: format!("flowlang:{}.bundle.{}.js", path, m.bundle.key),
      source_name: m.bundle.source_name.clone(),
      layout: m.layout,
      map: Some(m.bundle.clone()),
    });
  }
  let name = call.label.rsplit(':').next().filter(|n| is_identifier(n)).unwrap_or("command");
  // A body that awaits is an async function; any other stays synchronous, so
  // a JavaScript caller gets its value directly rather than a Promise.
  let asynchronous = if uses_await(&call.body) { "async " } else { "" };
  // The body starts on line 1, so the source's line numbers are the body's.
  let source = format!(
    "__flowlang.cmds[{key}] = {asynchronous}function {name}({params}) {{ {body}\n}}; __flowlang.hashes[{key}] = {hash};",
    key = key, asynchronous = asynchronous, name = name, params = call.params.join(", "), body = call.body, hash = hash
  );
  let script_name = format!("flowlang:{}.js", path);
  Ok(Registration {
    key,
    hash,
    source,
    source_name: script_name.clone(),
    script_name,
    layout: Layout { imports_first: 0, imports_lines: 0, body_first: 1, body_lines: call.body.lines().count().max(1) },
    map: None,
  })
}

thread_local! {
  /// Bundles' source maps by script name, for the engine's loader (engine
  /// thread only).
  static SOURCE_MAPS: std::cell::RefCell<std::collections::HashMap<String, Arc<Bundle>>> = Default::default();
}

/// Makes a registration's source map available before its script runs.
fn register_map(reg: &Registration) {
  if let Some(b) = &reg.map {
    SOURCE_MAPS.with(|m| { m.borrow_mut().insert(reg.script_name.clone(), b.clone()); });
  }
}

/// The engine's module loader: it loads no modules (bundles inline their
/// imports), and serves bundles' source maps so stack traces point into the
/// generated file.
struct MapLoader;

impl deno_core::ModuleLoader for MapLoader {
  fn resolve(&self, specifier: &str, referrer: &str, kind: deno_core::ResolutionKind) -> deno_core::ModuleResolveResponse {
    deno_core::NoopModuleLoader.resolve(specifier, referrer, kind)
  }
  fn load(
    &self,
    specifier: &deno_core::ModuleSpecifier,
    referrer: Option<&deno_core::ModuleLoadReferrer>,
    options: deno_core::ModuleLoadOptions,
  ) -> deno_core::ModuleLoadResponse {
    deno_core::NoopModuleLoader.load(specifier, referrer, options)
  }
  fn get_source_map(&self, file_name: &str) -> Option<std::borrow::Cow<'_, [u8]>> {
    SOURCE_MAPS.with(|m| m.borrow().get(file_name).map(|b| std::borrow::Cow::Owned(b.map.clone())))
  }
}

/// One argument per declared parameter, in order: the caller's own values -
/// objects and arrays as the same ndata (the command gets handles on them),
/// never copies.
fn positional_args(call: &JsCall) -> DataArray {
  let mut a = DataArray::new();
  for p in &call.params {
    Value::from_data(call.args.try_get_property(p).unwrap_or(Data::DNull)).push(&mut a);
  }
  a
}

thread_local! {
  /// True on the engine thread.
  static ON_ENGINE: Cell<bool> = const { Cell::new(false) };
  /// On a thread running a Rust, Python or flow command for a JavaScript
  /// caller: the engine thread, which waits inside that caller's
  /// `flowlang.call` and runs the JavaScript the command calls.
  static NESTED: std::cell::RefCell<Option<std::sync::mpsc::Sender<Event>>> = const { std::cell::RefCell::new(None) };
}

/// Runs a prepared call on the engine thread. Returns the success envelope
/// `{"a": value}`, or the error report.
pub fn run(call: &JsCall) -> Result<DataObject, String> {
  let reg = registration(call)?;
  let job = Job {
    label: call.label.clone(),
    reg,
    args: positional_args(call),
    flat: call.returntype == "FLAT",
    stdout_to_err: crate::pycmd::python_stdout_fd() == 2,
    timeout: call.timeout.or_else(default_timeout),
  };
  let (reply_tx, reply_rx) = std::sync::mpsc::channel();
  let mut msg = Msg { job, reply: reply_tx };
  if let Some(route) = NESTED.with(|n| n.borrow().clone()) {
    // A JavaScript command called the command running on this thread: its
    // engine is waiting for it, inside that call, and runs this one there.
    if route.send(Event::Js(msg)).is_err() {
      return Err(format!("{} was not run: the JavaScript command that called this one was stopped", call.label));
    }
    let value = recv_soon(&reply_rx, ENGINE_SPIN)
      .map_err(|_| format!("{} was not run: the JavaScript command that called this one was stopped", call.label))??;
    return Ok(ok_envelope(value));
  }
  if ON_ENGINE.with(|f| f.get()) {
    // Unreachable through flowlang.call (it runs other languages' commands
    // off the engine thread); kept so a mistake here fails rather than hangs.
    return Err(format!("{} cannot run on the JavaScript engine's own thread", call.label));
  }
  // The engine thread is gone only if it was killed from outside the panic
  // guard; start a new one once before giving up.
  for attempt in 0..2 {
    match engine().send(msg) {
      Ok(()) => break,
      Err(mpsc::error::SendError(m)) => {
        msg = m;
        restart_engine();
        if attempt == 1 { return Err("the JavaScript engine is not running".to_string()); }
      }
    }
  }
  let value = reply_rx.recv().map_err(|_| format!("the JavaScript engine stopped while running {}", call.label))??;
  Ok(ok_envelope(value))
}

/// What the engine thread is asked to run.
struct Job {
  label: String,
  reg: Registration,
  args: DataArray,
  flat: bool,
  stdout_to_err: bool,
  timeout: Option<Duration>,
}

struct Msg {
  job: Job,
  reply: std::sync::mpsc::Sender<Result<Value, String>>,
}

static ENGINE: Mutex<Option<mpsc::UnboundedSender<Msg>>> = Mutex::new(None);

fn engine() -> mpsc::UnboundedSender<Msg> {
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

fn panic_text(e: &Box<dyn std::any::Any + Send>) -> String {
  crate::callapi::panic_text(e)
}

fn spawn_engine() -> mpsc::UnboundedSender<Msg> {
  let (tx, mut rx) = mpsc::unbounded_channel::<Msg>();
  std::thread::Builder::new()
    .name("flowlang-js".to_string())
    .spawn(move || {
      ON_ENGINE.with(|f| f.set(true));
      let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("could not start the JavaScript engine's event loop");
      let watchdog = Watchdog::start();
      let mut engine: Option<Engine> = None;
      let mut pending: Vec<Pending> = Vec::new();
      // One loop serves everything: it takes new calls as they arrive, runs
      // the event loop for the calls that await (and for timers left
      // behind), and answers each call as its promise settles - so an
      // awaiting call never holds up the next one.
      rt.block_on(poll_fn(|cx| {
        let turn = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
          serve(cx, &mut rx, &mut engine, &mut pending, &watchdog)
        }));
        match turn {
          Ok(Poll::Ready(())) => Poll::Ready(()),
          Ok(Poll::Pending) => Poll::Pending,
          Err(e) => {
            // The isolate's state after a panic is unknown: start over.
            let why = panic_text(&e);
            abandon_waits();
            crate::jsndata::end_all_calls();
            for p in pending.drain(..) {
              let _ = p.reply.send(Err(format!("the JavaScript engine panicked and was restarted while running {}: {}", p.label, why)));
            }
            engine = None;
            watchdog.disarm();
            cx.waker().wake_by_ref();
            Poll::Pending
          }
        }
      }));
    })
    .expect("could not start the JavaScript engine thread");
  tx
}

/// A call whose promise has not settled yet.
struct Pending {
  label: String,
  reg: Registration,
  flat: bool,
  timeout: Option<Duration>,
  deadline: Option<Instant>,
  settled: std::pin::Pin<Box<dyn std::future::Future<Output = Result<v8::Global<v8::Value>, Box<deno_core::error::JsError>>>>>,
  sleep: Option<std::pin::Pin<Box<tokio::time::Sleep>>>,
  /// The watchdog stopped JavaScript while this call was waiting: V8 drops
  /// the microtasks queued behind stopped code, which may have been this
  /// call's.
  disrupted: bool,
  /// The call's id (its arguments and result slot, `jsndata`) and the
  /// array its result is written to.
  call_id: u32,
  out: DataArray,
  reply: std::sync::mpsc::Sender<Result<Value, String>>,
}

impl Pending {
  fn timed_out(&self) -> String {
    timed_out_text(&self.label, self.timeout)
  }
}

fn timed_out_text(label: &str, timeout: Option<Duration>) -> String {
  match timeout {
    Some(t) => format!("{} timed out after {} ms and was stopped", label, t.as_millis()),
    None => format!("{} was stopped", label),
  }
}

/// One pass of the engine loop. Returns Ready only when every sender is gone.
fn serve(
  cx: &mut std::task::Context,
  rx: &mut mpsc::UnboundedReceiver<Msg>,
  engine: &mut Option<Engine>,
  pending: &mut Vec<Pending>,
  watchdog: &Watchdog,
) -> Poll<()> {
  // New calls: run each one's synchronous part now.
  loop {
    match rx.poll_recv(cx) {
      Poll::Ready(None) => return Poll::Ready(()),
      Poll::Pending => break,
      Poll::Ready(Some(msg)) => {
        if engine.is_none() {
          match Engine::new(watchdog) {
            Ok(e) => *engine = Some(e),
            Err(e) => { let _ = msg.reply.send(Err(e)); continue; }
          }
        }
        let e = engine.as_mut().unwrap();
        match e.start(&msg.job, watchdog) {
          Started::Done(r) => { let _ = msg.reply.send(r); }
          Started::Waiting(settled, call_id, out) => {
            let deadline = msg.job.timeout.map(|t| Instant::now() + t);
            pending.push(Pending {
              label: msg.job.label.clone(),
              reg: msg.job.reg.clone(),
              flat: msg.job.flat,
              timeout: msg.job.timeout,
              deadline,
              settled,
              sleep: deadline.map(|d| Box::pin(tokio::time::sleep_until(tokio::time::Instant::from_std(d)))),
              disrupted: false,
              call_id,
              out,
              reply: msg.reply,
            });
          }
        }
        e.background = true;
      }
    }
  }
  let Some(e) = engine.as_mut() else { return Poll::Pending };
  if pending.is_empty() && !e.background { return Poll::Pending; }

  // One turn of the event loop, limited by the nearest deadline.
  let now = Instant::now();
  let limit = pending.iter().filter_map(|p| p.deadline).map(|d| d.saturating_duration_since(now))
    .chain(default_timeout())
    .min();
  watchdog.arm(limit);
  let turn = e.runtime.poll_event_loop(cx, PollEventLoopOptions::default());
  abandon_waits();
  let fired = e.disarm(watchdog);
  // Only a loop with nothing left to do is idle. An error - an uncaught
  // exception in a timer callback, or JavaScript the watchdog stopped -
  // says nothing about the work still queued behind it.
  let idle = match turn {
    Poll::Pending => false,
    Poll::Ready(Ok(())) => true,
    Poll::Ready(Err(err)) => {
      if !fired { eprintln!("[js] uncaught error in background work: {}", core_error_text(err, None)); }
      false
    }
  };
  if fired {
    if pending.iter().all(|p| p.deadline.map_or(true, |d| d > Instant::now())) {
      eprintln!("[js] background work ran past the time limit and was stopped");
    }
    for p in pending.iter_mut() { p.disrupted = true; }
  }

  // Answer the calls that settled, ran out of time, or can no longer settle.
  let mut i = 0;
  while i < pending.len() {
    let p = &mut pending[i];
    let answer = match p.settled.as_mut().poll(cx) {
      Poll::Ready(Ok(_)) => Some(finish(&p.label, p.flat, &p.out)),
      Poll::Ready(Err(js)) => Some(Err(report(&js, Some(&p.reg)))),
      Poll::Pending => {
        let late = p.deadline.map_or(false, |d| d <= Instant::now())
          || p.sleep.as_mut().map_or(false, |s| s.as_mut().poll(cx).is_ready());
        if late { Some(Err(p.timed_out())) }
        else if idle && p.disrupted {
          Some(Err(format!("{}: the command's promise is still pending and can never settle: its queued work was lost when the engine stopped JavaScript that ran past its time limit", p.label)))
        } else if idle {
          Some(Err(format!("{}: the command's promise is still pending and can never settle - it awaits something nothing will resolve (the event loop has no more work)", p.label)))
        } else { None }
      }
    };
    match answer {
      Some(r) => {
        let p = pending.swap_remove(i);
        crate::jsndata::end_call(p.call_id);
        let _ = p.reply.send(r);
      }
      None => i += 1,
    }
  }
  if idle && pending.is_empty() { e.background = false; }
  // Settling a promise can queue more work: look again.
  if !idle { cx.waker().wake_by_ref(); }
  Poll::Pending
}

/// Stops JavaScript that runs past its deadline. Armed around every stretch
/// of JavaScript execution; a separate thread terminates the isolate's
/// execution when the deadline passes.
struct Watchdog {
  inner: Arc<(Mutex<WatchState>, Condvar)>,
}

struct WatchState {
  deadline: Option<Instant>,
  fired: bool,
  handle: Option<v8::IsolateHandle>,
}

impl Watchdog {
  fn start() -> Watchdog {
    let inner = Arc::new((Mutex::new(WatchState { deadline: None, fired: false, handle: None }), Condvar::new()));
    let watcher = inner.clone();
    std::thread::Builder::new()
      .name("flowlang-js-watchdog".to_string())
      .spawn(move || {
        let (lock, cv) = &*watcher;
        let mut st = lock.lock().unwrap_or_else(|e| e.into_inner());
        loop {
          match st.deadline {
            None => { st = cv.wait(st).unwrap_or_else(|e| e.into_inner()); }
            Some(d) => {
              let now = Instant::now();
              if now >= d {
                if let Some(h) = &st.handle { h.terminate_execution(); }
                st.fired = true;
                st.deadline = None;
              } else {
                st = cv.wait_timeout(st, d - now).unwrap_or_else(|e| e.into_inner()).0;
              }
            }
          }
        }
      })
      .expect("could not start the JavaScript watchdog thread");
    Watchdog { inner }
  }

  fn set_handle(&self, handle: v8::IsolateHandle) {
    self.inner.0.lock().unwrap_or_else(|e| e.into_inner()).handle = Some(handle);
  }

  /// Starts the clock: `None` is no limit.
  fn arm(&self, timeout: Option<Duration>) {
    let (lock, cv) = &*self.inner;
    let mut st = lock.lock().unwrap_or_else(|e| e.into_inner());
    st.deadline = timeout.map(|t| Instant::now() + t);
    st.fired = false;
    cv.notify_all();
  }

  /// Stops the clock; true when it fired.
  fn disarm(&self) -> bool {
    let mut st = self.inner.0.lock().unwrap_or_else(|e| e.into_inner());
    st.deadline = None;
    std::mem::replace(&mut st.fired, false)
  }
}

use crate::jsweb::{op_flowlang_fetch, op_flowlang_now, op_flowlang_random, op_flowlang_url_parse, op_flowlang_url_set};
use crate::jsndata::{
  op_flowlang_call_args, op_flowlang_call_out, op_nd_delete, op_nd_get_bool, op_nd_get_bytes, op_nd_get_nd,
  op_nd_get_num, op_nd_get_str, op_nd_is_array, op_nd_keys, op_nd_kind, op_nd_len, op_nd_new, op_nd_set_bool,
  op_nd_set_bytes, op_nd_set_len, op_nd_set_nd, op_nd_set_null, op_nd_set_num, op_nd_set_str,
};
deno_core::extension!(flowlang_js, ops = [
  op_flowlang_call, op_flowlang_resume,
  op_flowlang_fetch, op_flowlang_now, op_flowlang_random, op_flowlang_url_parse, op_flowlang_url_set,
  op_flowlang_call_args, op_flowlang_call_out, op_nd_delete, op_nd_get_bool, op_nd_get_bytes, op_nd_get_nd,
  op_nd_get_num, op_nd_get_str, op_nd_is_array, op_nd_keys, op_nd_kind, op_nd_len, op_nd_new, op_nd_set_bool,
  op_nd_set_bytes, op_nd_set_len, op_nd_set_nd, op_nd_set_null, op_nd_set_num, op_nd_set_str,
]);

/// `flowlang.call` / `flowlang.api`: resolves the command and, unless it is
/// JavaScript, runs it on a worker thread while this one waits: `{"ok":
/// true}`, its value written to `out[0]`. When that command calls a
/// JavaScript command, the answer is `{"nested": {...}}` instead, which the
/// prelude runs here and answers through `op_flowlang_resume`.
/// The arguments come as ndata (`positional`, `named`), so a handle passed
/// along reaches the callee as the same object. A JavaScript callee comes
/// back as `{"js": {...}}`, its cast arguments in `out[0]`, for the prelude
/// to run in this isolate - the engine cannot take a new call while it is
/// inside this one. Failures are `{"err": {"kind": "lookup"|"args"|"failed",
/// "msg"}}`, which the prelude throws as ReferenceError, TypeError or
/// flowlang.CommandError.
#[deno_core::op2]
#[serde]
fn op_flowlang_call(
  #[string] lib: String,
  #[string] ctl: String,
  #[string] cmd: String,
  #[cppgc] positional: &crate::jsndata::Nd,
  #[cppgc] named: &crate::jsndata::Nd,
  #[cppgc] out: &crate::jsndata::Nd,
) -> serde_json::Value {
  use crate::jsndata::Nd;
  use serde_json::json;
  let err = |kind: &str, msg: String| json!({"err": {"kind": kind, "msg": msg}});
  let (Nd::Arr(positional), Nd::Obj(named), Nd::Arr(out)) = (positional, named, out) else {
    return err("failed", "flowlang.call: unexpected argument containers".to_string());
  };
  let target = match resolve_command(&lib, &ctl, &cmd) {
    Ok(t) => t,
    Err(msg) => return err("lookup", msg),
  };
  if positional.len() > target.params.len() {
    return err("args", format!(
      "{} takes {} argument(s) ({}) but {} positional were given",
      target.label, target.params.len(), target.params.join(", "), positional.len()
    ));
  }
  let mut values = DataObject::new();
  for (i, name) in target.params.iter().enumerate().take(positional.len()) {
    Value::from_data(positional.get_property(i)).put(&mut values, name);
  }
  for (k, d) in named.objects() {
    if values.has(&k) {
      return err("args", format!("{} got multiple values for argument '{}'", target.label, k));
    }
    Value::from_data(d).put(&mut values, &k);
  }

  let command = match std::panic::catch_unwind(|| crate::command::Command::new(&target.lib, &target.id)) {
    Ok(c) => c,
    Err(p) => return err("failed", format!("{} could not be loaded: {}", target.label, panic_text(&p))),
  };
  let mut out = out.clone();
  if is_js(&command.lang) {
    let cast = values.clone();
    if let Err(p) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| command.cast_params(cast))) {
      return err("args", format!("arguments for {} do not match its declared parameter types: {}", target.label, panic_text(&p)));
    }
    let call = match JSCmd::prepare(&target.lib, &target.id, values) {
      Ok(c) => c,
      Err(msg) => return err("args", format!("{}: {}", target.label, msg)),
    };
    let reg = match registration(&call) {
      Ok(r) => r,
      Err(msg) => return err("failed", msg),
    };
    register_map(&reg);
    out.push_array(positional_args(&call));
    let hash: String = serde_json::from_str(&reg.hash).unwrap_or_default();
    return json!({"js": {
      "label": call.label,
      "hash": hash,
      "source": reg.source,
      "scriptName": reg.script_name,
    }});
  }
  // Any other language runs on a worker thread while this one waits for it,
  // so a JavaScript command it calls can run here, in this isolate.
  let worker = IDLE.with(|w| w.borrow_mut().pop()).unwrap_or_else(Worker::spawn);
  let (events_tx, events) = std::sync::mpsc::channel();
  if worker.jobs.send(WorkerJob { target, values, events: events_tx }).is_err() {
    return err("failed", "flowlang.call: could not start the call".to_string());
  }
  let token = NEXT_WAIT.with(|n| { let t = n.get(); n.set(t.wrapping_add(1).max(1)); t });
  WAITS.with(|w| { w.borrow_mut().insert(token, Wait { events, worker, out, nested: None }); });
  wait_for(token)
}

/// Receives, spinning briefly before blocking: a handoff between the engine
/// thread and a worker happens on every call to another language's command,
/// and waking a blocked thread costs tens of microseconds where a spin costs
/// well under one.
fn recv_soon<T>(rx: &std::sync::mpsc::Receiver<T>, spin: Duration) -> Result<T, std::sync::mpsc::RecvError> {
  let start = Instant::now();
  loop {
    match rx.try_recv() {
      Ok(v) => return Ok(v),
      Err(std::sync::mpsc::TryRecvError::Disconnected) => return Err(std::sync::mpsc::RecvError),
      Err(std::sync::mpsc::TryRecvError::Empty) if start.elapsed() < spin => std::hint::spin_loop(),
      Err(std::sync::mpsc::TryRecvError::Empty) => return rx.recv(),
    }
  }
}

/// How long the engine thread spins waiting for a worker (and a worker for
/// the engine's answer to its JavaScript call): a quick command's time.
const ENGINE_SPIN: Duration = Duration::from_micros(50);
/// How long an idle worker spins for its next job: longer than the engine
/// takes to look a command up, so a JavaScript loop calling Rust stays warm.
const WORKER_SPIN: Duration = Duration::from_micros(250);

/// What a worker thread tells the engine thread waiting for it.
enum Event {
  /// The command called a JavaScript command: run it, answer `reply`.
  Js(Msg),
  /// The command returned.
  Done(Result<Value, CallFailure>),
}

/// A thread that runs Rust, Python and flow commands for JavaScript callers.
struct Worker {
  jobs: std::sync::mpsc::Sender<WorkerJob>,
}

struct WorkerJob {
  target: crate::callapi::Target,
  values: DataObject,
  events: std::sync::mpsc::Sender<Event>,
}

impl Worker {
  fn spawn() -> Worker {
    let (jobs, rx) = std::sync::mpsc::channel::<WorkerJob>();
    let spawned = std::thread::Builder::new().name("flowlang-js-call".to_string()).spawn(move || {
      // Ends when its Worker is dropped: an abandoned call, or the engine's end.
      while let Ok(job) = recv_soon(&rx, WORKER_SPIN) {
        NESTED.with(|n| *n.borrow_mut() = Some(job.events.clone()));
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_command(&job.target, job.values)))
          .unwrap_or_else(|p| Err(CallFailure::Failed(format!("{} panicked: {}", job.target.label, panic_text(&p)))));
        NESTED.with(|n| *n.borrow_mut() = None);
        let _ = job.events.send(Event::Done(r));
      }
    });
    if let Err(e) = spawned { eprintln!("[js] could not start a worker thread: {}", e); }
    Worker { jobs }
  }
}

/// A `flowlang.call` waiting for a command running on a worker.
struct Wait {
  events: std::sync::mpsc::Receiver<Event>,
  worker: Worker,
  /// Where the command's value goes.
  out: DataArray,
  /// The JavaScript command it called, while that runs here.
  nested: Option<Nested>,
}

struct Nested {
  label: String,
  reg: Registration,
  flat: bool,
  call_id: u32,
  out: DataArray,
  reply: std::sync::mpsc::Sender<Result<Value, String>>,
}

thread_local! {
  /// Calls waiting for a worker, on the engine thread, by token.
  static WAITS: std::cell::RefCell<std::collections::HashMap<u32, Wait>> = Default::default();
  static NEXT_WAIT: Cell<u32> = const { Cell::new(1) };
  /// Workers with nothing to do.
  static IDLE: std::cell::RefCell<Vec<Worker>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Blocks until the worker behind `token` returns, or calls a JavaScript
/// command: then `{"nested": {...}}` asks the prelude to run it here and
/// answer with `op_flowlang_resume`, which waits again.
fn wait_for(token: u32) -> serde_json::Value {
  use serde_json::json;
  let event = WAITS.with(|w| w.borrow().get(&token).map(|w| recv_soon(&w.events, ENGINE_SPIN)));
  let err = |kind: &str, msg: String| json!({"err": {"kind": kind, "msg": msg}});
  match event {
    None => err("failed", "flowlang.call: the call is no longer waiting".to_string()),
    Some(Err(_)) => {
      WAITS.with(|w| w.borrow_mut().remove(&token));
      err("failed", "flowlang.call: the worker running the command stopped".to_string())
    }
    Some(Ok(Event::Done(r))) => {
      let Some(wait) = WAITS.with(|w| w.borrow_mut().remove(&token)) else { return err("failed", String::new()) };
      IDLE.with(|i| i.borrow_mut().push(wait.worker));
      match r {
        Ok(value) => {
          let mut out = wait.out;
          value.push(&mut out);
          json!({"ok": true})
        }
        Err(CallFailure::BadArgs(m)) => err("args", m),
        Err(CallFailure::Failed(m)) => err("failed", m),
      }
    }
    Some(Ok(Event::Js(msg))) => {
      let job = msg.job;
      register_map(&job.reg);
      let (call_id, out) = crate::jsndata::begin_call(job.args.clone());
      let hash: String = serde_json::from_str(&job.reg.hash).unwrap_or_default();
      let ask = json!({"nested": {
        "token": token,
        "label": job.label,
        "hash": hash,
        "source": job.reg.source,
        "scriptName": job.reg.script_name,
        "callId": call_id,
      }});
      let nested = Nested { label: job.label, reg: job.reg, flat: job.flat, call_id, out, reply: msg.reply };
      WAITS.with(|w| { if let Some(w) = w.borrow_mut().get_mut(&token) { w.nested = Some(nested); } });
      ask
    }
  }
}

/// The prelude ran the JavaScript command a waiting call's command called:
/// `ok`, its value written to its call's result, or the `exception` it
/// threw. Answers the command, then waits for it again.
#[deno_core::op2]
#[serde]
fn op_flowlang_resume<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  #[smi] token: u32,
  ok: bool,
  exception: v8::Local<'s, v8::Value>,
) -> serde_json::Value {
  let nested = WAITS.with(|w| w.borrow_mut().get_mut(&token).and_then(|w| w.nested.take()));
  if let Some(n) = nested {
    let answer = if ok {
      finish(&n.label, n.flat, &n.out)
    } else {
      let js = deno_core::error::JsError::from_v8_exception(scope, exception);
      Err(report(&js, Some(&n.reg)))
    };
    crate::jsndata::end_call(n.call_id);
    let _ = n.reply.send(answer);
  }
  wait_for(token)
}

/// Drops the calls left waiting by JavaScript the watchdog stopped: their
/// commands' JavaScript calls fail, and their values are discarded.
fn abandon_waits() {
  let waits: Vec<Wait> = WAITS.with(|w| w.borrow_mut().drain().map(|(_, w)| w).collect());
  for w in waits {
    if let Some(n) = w.nested { crate::jsndata::end_call(n.call_id); }
  }
}

/// Installed once per isolate: the command table, the call trampoline,
/// timers, the `flowlang` API and a console that labels its lines and keeps
/// stdout clean under a protocol.
const PRELUDE: &str = r#"
(() => {
  const core = Deno.core;
  const ops = core.ops;
  const print = core.print;
  const state = { label: "", stdoutToErr: false };
  // jsndata.js's hooks, for this prelude only.
  const nd = globalThis.__flowlang_ndata;
  delete globalThis.__flowlang_ndata;

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
  const define = (name, value) =>
    Object.defineProperty(globalThis, name, { value, writable: true, configurable: true, enumerable: false });

  define("console", {
    log: (...a) => write(false, a),
    info: (...a) => write(false, a),
    debug: (...a) => write(false, a),
    warn: (...a) => write(true, a),
    error: (...a) => write(true, a),
    trace: (...a) => write(true, [...a, new Error("trace").stack]),
  });

  // Timers. A timer's callback runs on the engine's event loop: during a
  // call that awaits, or while the engine waits for its next call. What it
  // prints is labelled with the command that set it, whichever call is
  // running when it fires.
  const timer = (repeat) => (callback, delay, ...args) => {
    if (typeof callback !== "function") throw new TypeError("the callback must be a function");
    const label = state.label;
    const run = (...a) => {
      const outer = state.label;
      state.label = label;
      try { return callback(...a); } finally { state.label = outer; }
    };
    return core.createTimer(run, delay, args.length ? args : undefined, repeat, true, false);
  };
  const clear = (t) => { if (t && typeof t === "object") core.cancelTimer(t); };
  define("setTimeout", timer(false));
  define("setInterval", timer(true));
  define("clearTimeout", clear);
  define("clearInterval", clear);
  if (typeof globalThis.queueMicrotask !== "function") {
    define("queueMicrotask", (fn) => { Promise.resolve().then(fn); });
  }

  const cmds = Object.create(null);
  const hashes = Object.create(null);

  class CommandError extends Error {
    constructor(message) { super(message); this.name = "CommandError"; }
  }

  const ensure = (label, hash, source, scriptName) => {
    if (hashes[label] === hash) return;
    const [, error] = core.evalContext(source, scriptName);
    if (error) throw error.thrown;
  };

  // A JavaScript command called by a command that this isolate is waiting
  // for: [true] once its value is written, or [false, what it threw]. It
  // must finish now - the command waiting for it holds this thread - so one
  // that awaits fails here (the event loop cannot run until it returns).
  const nested = (n) => {
    const outer = state.label;
    state.label = n.label;
    try {
      ensure(n.label, n.hash, n.source, n.scriptName);
      let v = cmds[n.label](...nd.args(n.callId));
      if (v instanceof Promise) {
        const [settled, value] = core.getPromiseDetails(v);
        v.catch(() => {});
        if (settled === 0) {
          throw new CommandError(`${n.label} awaits, so it cannot run inside a call that a JavaScript command ` +
            "made to a Rust, Python or flow command: the event loop cannot run until that call returns. " +
            "Call it from JavaScript with flowlang.call, or make it synchronous (it was started and keeps running)");
        }
        if (settled === 2) throw value;
        v = value;
      }
      nd.result(n.callId, v, `return value of ${n.label}`);
      return [true, undefined];
    } catch (e) {
      return [false, e];
    } finally {
      state.label = outer;
    }
  };

  // Arguments cross as ndata: a handle passed along is the same object for
  // the callee, a plain value becomes ndata (as a dict does from Python).
  const invoke = (lib, ctl, cmd, positional, named) => {
    const label = `${lib}:${ctl}:${cmd}`;
    const pos = nd.arrayOf(positional, (i) => `argument ${i + 1} of ${label}`);
    const nam = nd.objectOf(named, (k) => `argument ${k} of ${label}`);
    const out = nd.newArray();
    let r = ops.op_flowlang_call(String(lib), String(ctl), String(cmd), pos, nam, out);
    // A Rust, Python or flow command calling a JavaScript command: run it
    // here, hand back its answer, and wait for the command again.
    while (r.nested) r = ops.op_flowlang_resume(r.nested.token, ...nested(r.nested));
    if (r.err) {
      if (r.err.kind === "lookup") throw new ReferenceError(r.err.msg);
      if (r.err.kind === "args") throw new TypeError(r.err.msg);
      throw new CommandError(r.err.msg);
    }
    if (r.ok) return nd.read(out, "0");
    // A JavaScript callee: its cast arguments, in declared order, are out[0].
    const j = r.js;
    ensure(j.label, j.hash, j.source, j.scriptName);
    const args = [...nd.read(out, "0")];
    const outer = state.label;
    state.label = j.label;
    try { return cmds[j.label](...args); }
    finally { state.label = outer; }
  };

  const level = (fn) => new Proxy(Object.create(null), {
    get: (_, name) => (typeof name === "string" && name !== "then") ? fn(name) : undefined,
  });
  const api = level((lib) => level((ctl) => level((cmd) =>
    (...args) => invoke(lib, ctl, cmd, args, {}))));

  define("flowlang", Object.freeze({
    call: (lib, ctl, cmd, args) => {
      if (args !== undefined && (args === null || typeof args !== "object" || Array.isArray(args))) {
        throw new TypeError("flowlang.call takes its arguments as an object of name: value");
      }
      return invoke(lib, ctl, cmd, [], args);
    },
    api,
    CommandError,
  }));

  // flow jstest: test() and assert for one test file at a time, handed to
  // the file rather than made global, so commands never see them.
  class AssertionError extends Error {
    constructor(message) { super(message); this.name = "AssertionError"; }
  }
  const show = (v) => {
    if (typeof v === "string") return JSON.stringify(v);
    try { const s = JSON.stringify(v); return s === undefined ? String(v) : s; } catch (_) { return String(v); }
  };
  const deepEqual = (a, b) => {
    if (Object.is(a, b)) return true;
    if (typeof a !== "object" || typeof b !== "object" || a === null || b === null) return false;
    if (Array.isArray(a) !== Array.isArray(b)) return false;
    const ka = Object.keys(a), kb = Object.keys(b);
    return ka.length === kb.length && ka.every((k) => Object.prototype.hasOwnProperty.call(b, k) && deepEqual(a[k], b[k]));
  };
  const matches = (e, expected) => {
    if (expected === undefined) return true;
    if (typeof expected === "function") return e instanceof expected;
    const text = String(e && e.message !== undefined ? e.message : e);
    return expected instanceof RegExp ? expected.test(text) : text.includes(String(expected));
  };
  const assert = (cond, message) => { if (!cond) throw new AssertionError(message || "assertion failed"); };
  assert.ok = assert;
  assert.equal = (actual, expected, message) => {
    if (!Object.is(actual, expected)) throw new AssertionError(message || `expected ${show(expected)}, got ${show(actual)}`);
  };
  assert.notEqual = (actual, expected, message) => {
    if (Object.is(actual, expected)) throw new AssertionError(message || `expected anything but ${show(expected)}`);
  };
  assert.deepEqual = (actual, expected, message) => {
    if (!deepEqual(actual, expected)) throw new AssertionError(message || `expected ${show(expected)}, got ${show(actual)}`);
  };
  assert.throws = (fn, expected, message) => {
    try { fn(); } catch (e) {
      if (!matches(e, expected)) throw new AssertionError(message || `threw ${e}, which does not match ${expected}`);
      return e;
    }
    throw new AssertionError(message || "expected an exception");
  };
  assert.rejects = async (promise, expected, message) => {
    try { await (typeof promise === "function" ? promise() : promise); } catch (e) {
      if (!matches(e, expected)) throw new AssertionError(message || `rejected with ${e}, which does not match ${expected}`);
      return e;
    }
    throw new AssertionError(message || "expected a rejection");
  };
  assert.AssertionError = AssertionError;
  Object.freeze(assert);

  let registered = null;
  const testing = Object.freeze({
    begin() {
      registered = [];
      const add = (skip) => (name, fn) => {
        if (typeof fn !== "function") throw new TypeError("test(name, fn): fn must be a function");
        registered.push({ name: String(name), fn, skip });
      };
      const test = add(false);
      test.skip = add(true);
      return { test, assert };
    },
    async finish() {
      const tests = registered || [];
      registered = null;
      const results = [];
      for (const t of tests) {
        if (t.skip) { results.push({ name: t.name, outcome: "skipped", ms: 0 }); continue; }
        const started = Date.now();
        try {
          await t.fn();
          results.push({ name: t.name, outcome: "passed", ms: Date.now() - started });
        } catch (e) {
          const error = e instanceof Error ? (e.stack || String(e)) : `${show(e)} (thrown)`;
          results.push({ name: t.name, outcome: "failed", ms: Date.now() - started, error });
        }
      }
      return results;
    },
  });

  Object.defineProperty(globalThis, "__flowlang", {
    value: Object.freeze({
      cmds,
      hashes,
      testing,
      // Runs call `callId`: its arguments (handles for objects and arrays)
      // come from jsndata, and its value is written back there.
      run(key, callId, stdoutToErr) {
        state.label = key;
        state.stdoutToErr = stdoutToErr;
        const what = `return value of ${key}`;
        let pending = false;
        try {
          const r = cmds[key](...nd.args(callId));
          if (r instanceof Promise) {
            // Keep the label while the command is still at work.
            pending = true;
            return r.then((v) => { nd.result(callId, v, what); })
              .finally(() => { if (state.label === key) state.label = ""; });
          }
          nd.result(callId, r, what);
        } finally {
          if (!pending) state.label = "";
        }
      },
    }),
  });
})();
"#;

struct Engine {
  runtime: JsRuntime,
  /// True while background work (timers, promises) may be pending.
  background: bool,
}

/// How a call's synchronous part ended.
enum Started {
  Done(Result<Value, String>),
  /// It returned a promise: the future settles with it; the call's id and
  /// result array.
  Waiting(std::pin::Pin<Box<dyn std::future::Future<Output = Result<v8::Global<v8::Value>, Box<deno_core::error::JsError>>>>>, u32, DataArray),
}

impl Engine {
  fn new(watchdog: &Watchdog) -> Result<Engine, String> {
    v8_flags();
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![flowlang_js::init()],
      module_loader: Some(std::rc::Rc::new(MapLoader)),
      ..Default::default()
    });
    runtime.execute_script("<flowlang web>", crate::jsweb::WEB_JS).map_err(|e| e.to_string())?;
    runtime.execute_script("<flowlang ndata>", crate::jsndata::NDATA_JS).map_err(|e| e.to_string())?;
    runtime.execute_script("<flowlang prelude>", PRELUDE).map_err(|e| e.to_string())?;
    watchdog.set_handle(runtime.v8_isolate().thread_safe_handle());
    Ok(Engine { runtime, background: false })
  }

  /// Stops the watchdog; when it fired, lets the isolate run again and
  /// says so.
  fn disarm(&mut self, watchdog: &Watchdog) -> bool {
    let fired = watchdog.disarm();
    if fired { self.runtime.v8_isolate().cancel_terminate_execution(); }
    fired
  }

  fn eval_bool(&mut self, src: String) -> Result<bool, String> {
    let v = self.runtime.execute_script("<flowlang check>", src).map_err(|e| e.to_string())?;
    deno_core::scope!(scope, &mut self.runtime);
    Ok(v8::Local::new(scope, v).is_true())
  }

  /// Installs the command if its source changed and runs its synchronous
  /// part, within the call's time limit.
  fn start(&mut self, job: &Job, watchdog: &Watchdog) -> Started {
    let reg = &job.reg;
    match self.eval_bool(format!("__flowlang.hashes[{}] === {}", reg.key, reg.hash)) {
      Err(e) => return Started::Done(Err(e)),
      Ok(true) => {}
      Ok(false) => {
        register_map(reg);
        if let Err(e) = self.runtime.execute_script(reg.script_name.clone(), reg.source.clone()) {
          return Started::Done(Err(report(&e, Some(reg))));
        }
      }
    }
    let (call_id, out) = crate::jsndata::begin_call(job.args.clone());
    watchdog.arm(job.timeout);
    let src = format!("__flowlang.run({}, {}, {})", reg.key, call_id, job.stdout_to_err);
    let result = self.runtime.execute_script("<flowlang call>", src);
    abandon_waits();
    let fired = self.disarm(watchdog);
    let result = match result {
      Ok(v) if !fired => v,
      Ok(_) => { crate::jsndata::end_call(call_id); return Started::Done(Err(timed_out_text(&job.label, job.timeout))); }
      Err(e) => {
        crate::jsndata::end_call(call_id);
        return Started::Done(Err(if fired { timed_out_text(&job.label, job.timeout) } else { report(&e, Some(reg)) }));
      }
    };
    let is_promise = {
      deno_core::scope!(scope, &mut self.runtime);
      v8::Local::new(scope, &result).is_promise()
    };
    if is_promise {
      Started::Waiting(Box::pin(self.runtime.resolve(result)), call_id, out)
    } else {
      crate::jsndata::end_call(call_id);
      Started::Done(finish(&job.label, job.flat, &out))
    }
  }
}

/// Applies `FLOWLANG_JS_V8_FLAGS` (V8 command-line flags, space-separated -
/// for profiling or tests, e.g. `--expose-gc`), once, before the first
/// isolate is made.
fn v8_flags() {
  static ONCE: std::sync::Once = std::sync::Once::new();
  ONCE.call_once(|| {
    if let Ok(flags) = std::env::var("FLOWLANG_JS_V8_FLAGS") {
      let mut args = vec!["flowlang".to_string()];
      args.extend(flags.split_whitespace().map(str::to_string));
      let rejected = deno_core::v8_set_flags(args);
      if rejected.len() > 1 { eprintln!("[js] FLOWLANG_JS_V8_FLAGS: V8 did not take {:?}", &rejected[1..]); }
    }
  });
}

/// A call's value, as its prelude wrote it (null if it wrote none).
fn finish(_label: &str, flat: bool, out: &DataArray) -> Result<Value, String> {
  let value = crate::jsndata::result_of(out).unwrap_or(Value::Scalar(Data::DNull));
  if flat && !matches!(value, Value::Object(_)) {
    let kind = match &value {
      Value::Array(_) => "an array",
      Value::Bytes(_) => "bytes",
      Value::Scalar(Data::DNull) => "null",
      Value::Scalar(Data::DString(_)) => "a string",
      Value::Scalar(Data::DBoolean(_)) => "a boolean",
      Value::Scalar(_) => "a number",
      Value::Object(_) => "an object",
    };
    return Err(format!("a FLAT command must return an object, got {}", kind));
  }
  Ok(value)
}

fn core_error_text(e: deno_core::error::CoreError, reg: Option<&Registration>) -> String {
  match *e.0 {
    deno_core::error::CoreErrorKind::Js(js) => report(&js, reg),
    other => other.to_string(),
  }
}

/// An exception as the caller sees it: the message, where in the command's
/// own body it happened, and the stack, without the engine's own frames.
fn report(e: &deno_core::error::JsError, reg: Option<&Registration>) -> String {
  let mut msg = e.exception_message.clone();
  if let Some(reg) = reg {
    let line = e.frames.iter()
      .find(|f| f.file_name.as_deref() == Some(reg.source_name.as_str()))
      .and_then(|f| f.line_number);
    if let Some(n) = line {
      let place = reg.layout.describe(n.max(0) as usize);
      if place == "the end of the body" { msg += " (at the end of the body)"; }
      else { msg += &format!(" ({})", place); }
    }
  }
  for l in e.to_string().lines() {
    let engine_frame = l.contains("<flowlang ") || l.contains("(ext:core/");
    if l.trim_start().starts_with("at ") && !engine_frame {
      msg += "\n";
      msg += l;
    }
  }
  msg
}

// ---- checking a command without running it -------------------------------------------------

/// Checks a JavaScript or TypeScript command without running it.
///
/// - Syntax: a plain JavaScript command compiles in V8 as the engine would
///   install it. One that imports modules, or is TypeScript, is bundled with
///   esbuild instead - which also resolves its imports, so a missing package
///   or one that needs Node's built-ins (`fs`, `http`, ...) is reported here.
/// - Types, when TypeScript's `tsc` is found: the generated file (JSDoc-typed
///   `// @ts-check` JavaScript, or the TypeScript itself, strict) against
///   flowlang.d.ts with `--lib es2022`, so names the engine does not provide
///   (`process`, `require`, `Buffer`) are errors and `flowlang.api` calls are
///   checked against the callee. Imports resolve from node_modules next to
///   data/, types included.
///
/// Problems are reported where the author wrote them ("body line N",
/// "imports line N"):
///
///   {"status": "ok", "command": "lib:ctl:cmd", "tsc": "ran" | "not installed" | "skipped",
///    "clean": bool,
///    "problems": [{"tool": "syntax"|"esbuild"|"tsc", "severity": "error", "where": "body line 3",
///                  "line": 3, "column": 5, "message": ...}]}
///
/// `tsc` is looked for as `$FLOWLANG_TSC`, then `node_modules/.bin/tsc`
/// next to `data/`, then `tsc` on the PATH; esbuild likewise
/// (`$FLOWLANG_ESBUILD`). `clean` is false when any problem has severity
/// "error". `status` is "err" only when the check itself cannot run.
pub fn check(lib: &str, id: &str) -> DataObject {
  check_with(lib, id, true)
}

/// `check` without tsc: the syntax check (or bundle) only - fast enough to
/// run on every save. `tsc` is "skipped".
pub fn check_quick(lib: &str, id: &str) -> DataObject {
  check_with(lib, id, false)
}

fn check_with(lib: &str, id: &str, type_check: bool) -> DataObject {
  let mut err = DataObject::new();
  err.put_string("status", "err");
  let located = std::panic::catch_unwind(|| {
    let (label, _) = JSCmd::locate(lib, id)?;
    let src = JsSource::read(&DataStore::new(), lib, id).ok_or_else(|| format!("{} has no JavaScript body", label))?;
    Ok::<_, String>((label, src))
  });
  let (label, src) = match located {
    Ok(Ok(x)) => x,
    Ok(Err(e)) => { err.put_string("msg", &e); return err; }
    Err(_) => { err.put_string("msg", &format!("no command {} in library {}", id, lib)); return err; }
  };
  let store = DataStore::new();
  let layout = gen::layout(&src);

  let mut problems = DataArray::new();
  // `place`: the author's name for the line ("body line 3"), when known.
  let mut problem = |tool: &str, place: Option<String>, line: Option<i64>, column: Option<i64>, message: &str| {
    let mut p = DataObject::new();
    p.put_string("tool", tool);
    p.put_string("severity", "error");
    p.put_string("where", place.as_deref().unwrap_or("the command"));
    if let Some(n) = line { p.put_int("line", n); }
    if let Some(c) = column { p.put_int("column", c); }
    p.put_string("message", message);
    problems.push_object(p);
  };
  // A generated-file line in the author's terms, and the line number within
  // its part (imports or body).
  let place = |file_line: usize| -> (String, Option<i64>) {
    let d = layout.describe(file_line);
    let n = d.rsplit(' ').next().and_then(|n| n.parse::<i64>().ok());
    (d, n)
  };

  let mut syntax_ok = true;
  if src.needs_bundle() {
    let mut parts = label.splitn(3, ':').skip(1);
    let (ctl, cmd) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    match gen::bundle(&store, &store.get_lib_root(lib).join("src"), lib, ctl, cmd, &src) {
      Ok(_) => {}
      Err(BundleError::Failed(ds)) => {
        syntax_ok = false;
        for d in ds {
          let (where_, n) = match d.line { Some(l) => { let (w, n) = place(l); (Some(w), n) } None => (None, None) };
          problem("esbuild", where_, n, d.column.map(|c| c as i64 + 1), &d.full(None));
        }
      }
      Err(e) => { syntax_ok = false; problem("esbuild", None, None, None, &e.text(&layout)); }
    }
  } else {
    let call = JsCall {
      label: label.clone(),
      params: src.params.iter().map(|(n, _)| n.clone()).collect(),
      body: src.body.clone(),
      returntype: src.returntype.clone(),
      args: DataObject::new(),
      timeout: None,
      module: None,
    };
    match registration(&call) {
      Err(msg) => { syntax_ok = false; problem("syntax", None, None, None, &msg); }
      Ok(reg) => {
        if let Some((message, line, column)) = syntax_problem(&call, &reg) {
          syntax_ok = false;
          // V8's lines are the installed source's: body line N is line N.
          let where_ = line.map(|l| reg.layout.describe(l.max(0) as usize));
          let n = if where_.as_deref().map_or(false, |w| w.starts_with("body line")) { line } else { None };
          problem("syntax", where_, n, column, &message);
        }
      }
    }
  }

  let tsc_state = if !type_check || !syntax_ok {
    "skipped".to_string()
  } else {
    let file = format!("command.{}", src.extension());
    // A command that imports packages is checked next to data/, so its
    // imports (and their types) resolve from node_modules there.
    let parent = store.root.canonicalize().ok().and_then(|r| r.parent().map(|p| p.to_path_buf()));
    let base = match (&parent, src.needs_bundle()) {
      (Some(p), true) => p.clone(),
      _ => std::env::temp_dir(),
    };
    let dir = base.join(format!(".flowlang_check_js_{}_{:x}", std::process::id(), {
      let mut h = DefaultHasher::new(); (&label, Instant::now()).hash(&mut h); h.finish()
    }));
    let _ = std::fs::create_dir_all(&dir);
    // Imported commands are checked through their generated files; their
    // own problems are their own check's to report.
    let modules = {
      let mut parts = label.splitn(3, ':');
      let own = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or(""));
      gen::shared_modules(&store, &src, own).unwrap_or_default()
    };
    gen::write_modules(&store, &modules);
    let written = std::fs::write(dir.join("flowlang.d.ts"), gen::flowlang_dts(&store))
      .and_then(|_| std::fs::write(dir.join(&file), gen::command_file(&src, "flowlang.d.ts")))
      .and_then(|_| std::fs::write(dir.join("tsconfig.json"), tsconfig(&src, &file, &modules)));
    let state = match written {
      Err(e) => { let _ = std::fs::remove_dir_all(&dir); err.put_string("msg", &format!("could not write the check files: {}", e)); return err; }
      Ok(()) => match run_tsc(&store, &dir, &file) {
        None => "not installed".to_string(),
        Some(Err(e)) => { let _ = std::fs::remove_dir_all(&dir); err.put_string("msg", &e); return err; }
        Some(Ok(diagnostics)) => {
          for (line, column, message) in diagnostics {
            let (where_, n) = place(line.max(0) as usize);
            problem("tsc", Some(where_), n, Some(column), &message);
          }
          "ran".to_string()
        }
      },
    };
    let _ = std::fs::remove_dir_all(&dir);
    state
  };

  let clean = problems.len() == 0;
  let mut o = DataObject::new();
  o.put_string("status", "ok");
  o.put_string("command", &label);
  o.put_string("tsc", &tsc_state);
  o.put_boolean("clean", clean);
  o.put_array("problems", problems);
  o
}

/// The check's tsconfig: JavaScript is checked loosely (`checkJs`, not
/// strict), TypeScript strictly; a module resolves its imports the way a
/// bundler does.
fn tsconfig(src: &JsSource, file: &str, modules: &[gen::SharedModule]) -> String {
  let paths = if modules.is_empty() { String::new() } else { format!(",\n    \"paths\": {}", gen::module_paths(modules).to_string()) };
  format!(r#"{{
  "compilerOptions": {{
    "target": "es2022",
    "lib": ["es2022"],
    "types": [],
    "module": "esnext",
    "moduleResolution": "bundler",
    "esModuleInterop": true,
    "allowSyntheticDefaultImports": true,
    "allowJs": true,
    "checkJs": true,
    "noEmit": true,
    "strict": {strict},
    "noImplicitAny": {strict},
    "skipLibCheck": true{paths}
  }},
  "files": ["flowlang.d.ts", "{file}"]
}}
"#, strict = src.is_typescript(), file = file, paths = paths)
}

/// Compiles the command as the engine would install it, in a short-lived
/// isolate on a thread of its own (so it is safe from any thread, the
/// engine's included). Returns the message and body line of a syntax error.
fn syntax_problem(call: &JsCall, reg: &Registration) -> Option<(String, Option<i64>, Option<i64>)> {
  let asynchronous = if uses_await(&call.body) { "async " } else { "" };
  let name = call.label.rsplit(':').next().filter(|n| is_identifier(n)).unwrap_or("command");
  let source = format!("void ({}function {}({}) {{ {}\n}});", asynchronous, name, call.params.join(", "), call.body);
  let script_name = reg.script_name.clone();
  std::thread::scope(|s| {
    s.spawn(move || {
      // deno_core's isolates need a tokio runtime context.
      let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().ok()?;
      let _guard = rt.enter();
      v8_flags();
      let mut js = JsRuntime::new(RuntimeOptions::default());
      match js.execute_script(script_name.clone(), source) {
        Ok(_) => None,
        Err(e) => {
          let frame = e.frames.iter().find(|f| f.file_name.as_deref() == Some(script_name.as_str()));
          Some((e.exception_message.clone(), frame.and_then(|f| f.line_number), frame.and_then(|f| f.column_number)))
        }
      }
    }).join().unwrap_or_else(|_| Some(("the syntax check panicked".to_string(), None, None)))
  })
}

/// Runs tsc in `dir`: `None` when no tsc is found, else its diagnostics for
/// `file` as (file line, column, message).
fn run_tsc(store: &DataStore, dir: &std::path::Path, file: &str) -> Option<Result<Vec<(i64, i64, String)>, String>> {
  let mut candidates: Vec<std::path::PathBuf> = Vec::new();
  if let Ok(t) = std::env::var("FLOWLANG_TSC") { candidates.push(t.into()); }
  if let Ok(root) = store.root.canonicalize() {
    if let Some(parent) = root.parent() { candidates.push(parent.join("node_modules").join(".bin").join("tsc")); }
  }
  candidates.push("tsc".into());
  let prefix = format!("{}(", file);
  for exe in candidates {
    if exe.components().count() > 1 && !exe.exists() { continue; }
    let out = std::process::Command::new(&exe)
      .args(["-p", "tsconfig.json", "--pretty", "false"])
      .current_dir(dir)
      .output();
    let out = match out {
      Ok(o) => o,
      Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
      Err(e) => return Some(Err(format!("could not start {}: {}", exe.display(), e))),
    };
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let mut found: Vec<(i64, i64, String)> = Vec::new();
    // In another file - an imported command's - which its own check reports.
    let mut elsewhere = false;
    let other_file = |l: &str| !l.starts_with(' ') && l.split_once("): error TS").map_or(false, |(head, _)| {
      head.rsplit_once('(').map_or(false, |(f, pos)| !f.is_empty() && pos.split(',').all(|n| n.parse::<u32>().is_ok()))
    });
    for l in text.lines() {
      // command.js(12,5): error TS2304: Cannot find name 'process'.
      if let Some(rest) = l.strip_prefix(prefix.as_str()) {
        let parsed = rest.split_once("): ").and_then(|(pos, msg)| {
          let (line, col) = pos.split_once(',')?;
          Some((line.parse().ok()?, col.parse().ok()?, msg.split_once(": ").map(|(_, m)| m).unwrap_or(msg).to_string()))
        });
        if let Some(p) = parsed { found.push(p); elsewhere = false; continue; }
      }
      if other_file(l) { elsewhere = true; continue; }
      if l.starts_with(' ') && elsewhere { continue; }
      if l.starts_with(' ') {
        // A continuation of the previous message.
        if let Some(last) = found.last_mut() { last.2.push('\n'); last.2.push_str(l.trim_end()); }
      } else if !l.trim().is_empty() && !out.status.success() && found.is_empty() && !l.starts_with(file) {
        return Some(Err(format!("{} failed: {}", exe.display(), text.trim())));
      }
    }
    return Some(Ok(found));
  }
  None
}

// ---- flow jstest ---------------------------------------------------------------------------

/// `flow jstest [files or directories]`: runs JavaScript test files in the
/// engine commands run in, so tests call any command through `flowlang.call`
/// / `flowlang.api` and see the same globals. A directory is searched for
/// `*.test.js` (skipping `node_modules`, `target` and hidden directories);
/// with no arguments, the current directory is. Each file gets `test(name,
/// fn)` (`fn` may be async; `test.skip` skips) and `assert` (`assert(cond)`,
/// `equal`, `notEqual`, `deepEqual`, `throws`, `rejects`) as locals; tests
/// run in order, and each file has the engine's time limit. Prints one line
/// per test and returns the exit status: 0 all passed, 1 a test or file
/// failed, 2 nothing to run.
pub fn run_tests(args: Vec<String>) -> i32 {
  let roots: Vec<std::path::PathBuf> = if args.is_empty() { vec![".".into()] } else { args.iter().map(Into::into).collect() };
  let mut files = Vec::new();
  for r in &roots {
    if r.is_file() { files.push(r.clone()); }
    else if r.is_dir() { find_tests(r, &mut files); }
    else { eprintln!("jstest: no such file or directory: {}", r.display()); return 2; }
  }
  files.sort();
  files.dedup();
  if files.is_empty() {
    eprintln!("jstest: no *.test.js files under {}", roots.iter().map(|r| r.display().to_string()).collect::<Vec<_>>().join(", "));
    return 2;
  }
  let (mut passed, mut failed, mut skipped) = (0, 0, 0);
  for file in &files {
    let shown = file.display().to_string();
    println!("{}", shown);
    let source = match std::fs::read_to_string(file) {
      Ok(s) => s,
      Err(e) => { println!("  ✗ could not be read: {}", e); failed += 1; continue; }
    };
    let rel = shown.trim_start_matches("./").trim_end_matches(".js").replace(':', "_");
    // The file starts on body line 1, so its line numbers are the file's own.
    let call = JsCall {
      label: format!("jstest:{}", rel),
      params: vec![],
      body: format!("const {{ test, assert }} = __flowlang.testing.begin(); {}\n;return await __flowlang.testing.finish();", source),
      returntype: "JSONArray".to_string(),
      args: DataObject::new(),
      timeout: None,
      module: None,
    };
    match run(&call) {
      Err(e) => { println!("  ✗ the file failed: {}", indent(&clean_stack(&e))); failed += 1; }
      Ok(env) => {
        let results = env.get_array("a");
        if results.len() == 0 { println!("  (no tests)"); }
        for r in results.objects() {
          let r = r.object();
          let name = r.get_string("name");
          match r.get_string("outcome").as_str() {
            "passed" => { passed += 1; println!("  ✓ {} ({} ms)", name, r.get_int("ms")); }
            "skipped" => { skipped += 1; println!("  - {} (skipped)", name); }
            _ => {
              failed += 1;
              println!("  ✗ {} ({} ms)\n      {}", name, r.get_int("ms"), indent(&clean_stack(&r.get_string("error"))));
            }
          }
        }
      }
    }
  }
  println!("\n{} passed, {} failed, {} skipped", passed, failed, skipped);
  if failed > 0 { 1 } else if passed + skipped == 0 { 2 } else { 0 }
}

fn find_tests(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
  let Ok(entries) = std::fs::read_dir(dir) else { return };
  for e in entries.flatten() {
    let path = e.path();
    let name = e.file_name().to_string_lossy().to_string();
    if path.is_dir() {
      if name.starts_with('.') || name == "node_modules" || name == "target" { continue; }
      find_tests(&path, out);
    } else if name.ends_with(".test.js") {
      out.push(path);
    }
  }
}

/// A test's stack as the file's author reads it: the file's own path and
/// line numbers, without the engine's or the harness's frames.
fn clean_stack(stack: &str) -> String {
  stack.lines()
    .filter(|l| !(l.trim_start().starts_with("at ")
      && (l.contains("<flowlang") || l.contains("(ext:core/") || l.contains("[as jstest:"))))
    .map(|l| l.replace("flowlang:jstest/", "").replace("(body line ", "(line "))
    .collect::<Vec<_>>()
    .join("\n")
}

fn indent(s: &str) -> String {
  s.replace('\n', "\n      ")
}
