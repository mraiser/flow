//! JavaScript commands: the time limit. Its own binary (its own engine):
//! stopping runaway JavaScript discards the microtasks queued behind it, so
//! these tests must not share an engine with tests whose calls await.
#![cfg(feature = "javascript_runtime")]

use flowlang::jscmd::{run, JsCall};
use ndata::dataobject::DataObject;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Once;
use std::time::{Duration, Instant};

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
    timeout: None,
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

fn ret_within(body: &str, ms: u64) -> Result<DataObject, String> {
  let mut c = call(&[], body, "Any", obj());
  c.timeout = Some(Duration::from_millis(ms));
  run(&c)
}

// ---- time limits ---------------------------------------------------------------------------

#[test]
fn a_runaway_loop_is_stopped_and_the_engine_carries_on() {
  let started = Instant::now();
  let e = err(ret_within("while (true) {}", 300));
  assert!(e.contains("timed out after 300 ms"), "{}", e);
  assert!(started.elapsed() < Duration::from_secs(5));
  assert_eq!(json(&ret("return 'next'").unwrap()), v(r#"{"v":"next"}"#));
}

#[test]
fn a_slow_await_is_stopped_too() {
  let started = Instant::now();
  let e = err(ret_within("await new Promise((r) => setTimeout(r, 60000));\nreturn 1", 300));
  assert!(e.contains("timed out after 300 ms"), "{}", e);
  assert!(started.elapsed() < Duration::from_secs(5));
  assert_eq!(json(&ret("return 'next'").unwrap()), v(r#"{"v":"next"}"#));
}

#[test]
fn a_loop_inside_an_async_command_is_stopped() {
  let e = err(ret_within("await null;\nwhile (true) {}", 300));
  assert!(e.contains("timed out"), "{}", e);
  assert_eq!(json(&ret("return 'next'").unwrap()), v(r#"{"v":"next"}"#));
}

