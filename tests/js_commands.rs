//! JavaScript commands: the result envelope, the value conversion rules, the
//! error reports, and the engine's behaviour across calls and threads.
//!
//!   cargo test --features=javascript_runtime --test js_commands
//!
//! A JavaScript command must look exactly like a Rust or Python command to
//! its callers: `{"a": value}` on success, `{"a": {"status": "err", "msg"}}`
//! on failure. Before, it returned `{"status", "msg"|"data"}`, ran a V8
//! isolate shared across threads behind `unsafe impl Send`, and a single
//! syntax error poisoned the engine's lock so every later call panicked.
#![cfg(feature = "javascript_runtime")]

use flowlang::jscmd::{run, JsCall};
use ndata::dataobject::DataObject;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Once;

static INIT: Once = Once::new();
static SEQ: AtomicUsize = AtomicUsize::new(0);

fn init() {
  INIT.call_once(|| { flowlang::init("data"); });
}

fn label() -> String {
  format!("jstest{}:ctl:cmd{}", std::process::id(), SEQ.fetch_add(1, Ordering::SeqCst))
}

fn call(params: &[&str], body: &str, returntype: &str, args: DataObject) -> JsCall {
  init();
  JsCall {
    label: label(),
    params: params.iter().map(|s| s.to_string()).collect(),
    body: body.to_string(),
    returntype: returntype.to_string(),
    args,
  }
}

fn ret(body: &str) -> Result<DataObject, String> {
  run(&call(&[], body, "Any", obj()))
}

/// A new object, with ndata initialized first.
fn obj() -> DataObject {
  init();
  DataObject::new()
}

/// The envelope's value as `{"v": ...}`, read while the envelope is alive,
/// parsed: ndata objects do not keep key order.
fn json(env: &DataObject) -> serde_json::Value {
  let mut o = DataObject::new();
  o.set_property("v", env.get_property("a"));
  serde_json::from_str(&o.to_string()).unwrap()
}

fn v(s: &str) -> serde_json::Value {
  serde_json::from_str(s).unwrap()
}

fn err(r: Result<DataObject, String>) -> String {
  match r {
    Ok(o) => panic!("expected an error, got {}", o.to_string()),
    Err(e) => e,
  }
}

// ---- conversion -------------------------------------------------------------------------

#[test]
fn scalars_keep_their_types() {
  assert_eq!(json(&ret("return true").unwrap()), v(r#"{"v":true}"#));
  assert_eq!(json(&ret("return 0").unwrap()), v(r#"{"v":0}"#));
  assert_eq!(json(&ret("return -3").unwrap()), v(r#"{"v":-3}"#));
  assert_eq!(json(&ret("return 1.5").unwrap()), v(r#"{"v":1.5}"#));
  assert_eq!(json(&ret("return 'hi'").unwrap()), v(r#"{"v":"hi"}"#));
  assert_eq!(json(&ret("return null").unwrap()), v(r#"{"v":null}"#));
}

#[test]
fn undefined_is_null() {
  assert_eq!(json(&ret("return undefined").unwrap()), v(r#"{"v":null}"#));
  assert_eq!(json(&ret("let x = 1;").unwrap()), v(r#"{"v":null}"#));
}

#[test]
fn objects_and_arrays_nest() {
  assert_eq!(
    json(&ret("return {n: 1, list: [1, 'two', {three: 3}], none: null}").unwrap()),
    v(r#"{"v":{"n":1,"list":[1,"two",{"three":3}],"none":null}}"#)
  );
  assert_eq!(json(&ret("return [[1, 2], []]").unwrap()), v(r#"{"v":[[1,2],[]]}"#));
}

#[test]
fn values_that_do_not_convert_are_errors() {
  for (body, kind) in [
    ("return (async () => 1)()", "Promise"),
    ("return function () {}", "function"),
    ("return 10n", "BigInt"),
    ("return Symbol('s')", "symbol"),
    ("return new Map()", "Map"),
    ("return new Set()", "Set"),
  ] {
    let e = err(ret(body));
    assert!(e.contains("could not be converted") && e.contains(kind), "{}: {}", body, e);
  }
}

#[test]
fn flat_must_return_an_object() {
  let ok = run(&call(&[], "return {status: 'ok', x: 1}", "FLAT", obj())).unwrap();
  assert_eq!(json(&ok), v(r#"{"v":{"status":"ok","x":1}}"#));
  let e = err(run(&call(&[], "return [1]", "FLAT", obj())));
  assert!(e.contains("FLAT command must return an object, got an array"), "{}", e);
}

// ---- arguments ----------------------------------------------------------------------------

#[test]
fn arguments_arrive_by_name_in_declared_order() {
  let mut args = obj();
  args.put_int("b", 2);
  args.put_string("a", "x");
  let mut o = obj();
  o.put_int("k", 7);
  args.put_object("c", o);
  let r = run(&call(&["a", "b", "c"], "return a + ':' + (b * 10) + ':' + c.k", "String", args)).unwrap();
  assert_eq!(json(&r), v(r#"{"v":"x:20:7"}"#));
}

#[test]
fn awkward_strings_cross_intact() {
  let mut args = obj();
  let s = "quote\" back\\slash \u{2028} line\nnew </script> ünï";
  args.put_string("s", s);
  let r = run(&call(&["s"], "return s", "String", args)).unwrap();
  assert_eq!(r.get_string("a"), s);
}

#[test]
fn a_parameter_name_must_be_an_identifier() {
  let mut args = obj();
  args.put_int("a-b", 1);
  let e = err(run(&call(&["a-b"], "return 1", "Any", args)));
  assert!(e.contains("not a JavaScript identifier"), "{}", e);
}

// ---- errors -------------------------------------------------------------------------------

#[test]
fn an_exception_names_its_body_line() {
  let e = err(ret("let o = {};\nlet p = 2;\nreturn o.missing.deeper;"));
  assert!(e.contains("TypeError"), "{}", e);
  assert!(e.contains("(body line 3)"), "{}", e);
  // The stack's line numbers are the body's own, and the engine's frames
  // are left out.
  assert!(e.contains("/cmd") && e.contains(".js:3:"), "{}", e);
  assert!(!e.contains("<flowlang"), "{}", e);
}

#[test]
fn a_thrown_error_keeps_its_message() {
  let e = err(ret("throw new Error('nope')"));
  assert!(e.contains("nope") && e.contains("(body line 1)"), "{}", e);
}

#[test]
fn a_syntax_error_is_reported_and_the_engine_keeps_working() {
  let c = call(&[], "return (", "Any", obj());
  let e = err(run(&c));
  assert!(e.contains("SyntaxError") && e.contains("(at the end of the body)"), "{}", e);
  // The same command, fixed: the new body runs, never the old one.
  let mut fixed = c.clone();
  fixed.body = "return 42".to_string();
  assert_eq!(json(&run(&fixed).unwrap()), v(r#"{"v":42}"#));
  // And everything else still runs.
  assert_eq!(json(&ret("return 'still alive'").unwrap()), v(r#"{"v":"still alive"}"#));
}

#[test]
fn an_edited_body_replaces_the_old_one() {
  let c = call(&[], "return 1", "Any", obj());
  assert_eq!(json(&run(&c).unwrap()), v(r#"{"v":1}"#));
  let mut edited = c.clone();
  edited.body = "return 2".to_string();
  assert_eq!(json(&run(&edited).unwrap()), v(r#"{"v":2}"#));
}

#[test]
fn renamed_parameters_take_effect_with_the_same_body() {
  let mut args = obj();
  args.put_int("x", 1);
  args.put_int("y", 2);
  let c = call(&["x", "y"], "return typeof x + typeof y", "String", args.clone());
  assert_eq!(run(&c).unwrap().get_string("a"), "numbernumber");
  let mut renamed = c.clone();
  renamed.params = vec!["y".to_string()];
  assert_eq!(run(&renamed).unwrap().get_string("a"), "undefinednumber");
}

// ---- state and threads --------------------------------------------------------------------

#[test]
fn global_state_persists_between_calls() {
  let c = call(&[], "globalThis.__count = (globalThis.__count || 0) + 1; return globalThis.__count", "Integer", obj());
  let first = run(&c).unwrap().get_int("a");
  let second = run(&c).unwrap().get_int("a");
  assert_eq!(second, first + 1);
}

#[test]
fn calls_from_many_threads_all_answer() {
  init();
  let c = call(&["n"], "let s = 0; for (let i = 0; i < 1000; i++) s += n; return s", "Integer", obj());
  let threads: Vec<_> = (0..16).map(|n| {
    let mut c = c.clone();
    std::thread::spawn(move || {
      let mut args = obj();
      args.put_int("n", n);
      c.args = args;
      (n, run(&c).unwrap().get_int("a"))
    })
  }).collect();
  for t in threads {
    let (n, got) = t.join().unwrap();
    assert_eq!(got, n * 1000);
  }
}
