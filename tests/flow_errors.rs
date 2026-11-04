//! A failed flow says what went wrong and where.
//!
//! `Code::execute` and `Command::execute` return `CodeException::Fail(FlowError)`: a code, a
//! message, the path of ops (with iterations) from the root frame down to the failure, and for
//! a `fail` rule, the failure of the local or flow command that made it fire. These tests pin
//! the payload, its one-line text and its `{"status": "err", ...}` envelope, and how the CLI,
//! the MCP server, `execute_command` and `try_cast_params` hand it on.
//!
//! One #[test] on purpose: the store and ndata globals initialize once per process.

use flowlang::case::Case;
use flowlang::code::{Code, CodeException, FlowError};
use flowlang::command::Command;
use ndata::dataobject::DataObject;
use std::io::Write;
use std::process::{Command as Process, Stdio};

const N: &str = r#"{"mode":"regular","type":"Any","x":0}"#;
const L: &str = r#"{"mode":"list","type":"Any","x":0}"#;
const P: &str = r#""pos":{"x":0,"y":0,"z":0},"width":1"#;

fn run(body: &str, args: &str) -> Result<DataObject, CodeException> {
  Code::new(Case::from_data(DataObject::from_string(body))).execute(DataObject::from_string(args))
}
fn failure(body: &str, args: &str) -> FlowError {
  match run(body, args) {
    Err(CodeException::Fail(e)) => e,
    other => { let _ = std::panic::take_hook(); panic!("expected the flow to fail, got {:?}", other.map(|o| o.to_string())) }
  }
}

/// A list-mode local "sums" adding 12 to each element of each sublist; an element that is not a
/// list fails the inner list op.
fn sums_body() -> String {
  format!(r#"{{"input":{{"xs":{n}}},"output":{{"r":{n}}},"cmds":[
    {{"type":"local","name":"sums","in":{{"b":{l}}},"out":{{"q":{l}}},{p},"localdata":
      {{"input":{{"b":{n}}},"output":{{"q":{n}}},"cmds":[
        {{"type":"constant","ctype":"int","name":"12","in":{{}},"out":{{"a":{n}}},{p}}},
        {{"type":"primitive","name":"+","in":{{"a":{n},"b":{l}}},"out":{{"a":{l}}},{p}}}],
       "cons":[{{"src":[0,"a"],"dest":[1,"a"]}},{{"src":[-1,"b"],"dest":[1,"b"]}},{{"src":[1,"a"],"dest":[-2,"q"]}}]}}}}],
    "cons":[{{"src":[-1,"xs"],"dest":[0,"b"]}},{{"src":[0,"q"],"dest":[-2,"r"]}}]}}"#, n = N, l = L, p = P)
}

/// A local "check" whose match op fails unless v is "ok"; the local's own fail rule re-raises it.
fn check_body() -> String {
  format!(r#"{{"input":{{"v":{n}}},"output":{{}},"cmds":[
    {{"type":"local","name":"check","in":{{"v":{n}}},"out":{{}},{p},"condition":{{"rule":"fail","value":false}},"localdata":
      {{"input":{{"v":{n}}},"output":{{}},"cmds":[
        {{"type":"match","ctype":"string","name":"ok","in":{{"a":{n}}},"out":{{}},{p},"condition":{{"rule":"fail","value":false}}}}],
       "cons":[{{"src":[-1,"v"],"dest":[0,"a"]}}]}}}}],
    "cons":[{{"src":[-1,"v"],"dest":[0,"v"]}}]}}"#, n = N, p = P)
}

/// check_body inside a list-mode local "outer" over vs, whose fail rule re-raises again.
fn outer_body() -> String {
  format!(r#"{{"input":{{"vs":{n}}},"output":{{}},"cmds":[
    {{"type":"local","name":"outer","in":{{"v":{l}}},"out":{{}},{p},"condition":{{"rule":"fail","value":false}},"localdata":{inner}}}],
    "cons":[{{"src":[-1,"vs"],"dest":[0,"v"]}}]}}"#, n = N, l = L, p = P, inner = check_body())
}

/// A wire cycle (I-2): op 0 waits for op 1 and op 1 for op 0, so the case can never finish.
fn stuck_body() -> String {
  format!(r#"{{"input":{{}},"output":{{"r":{n}}},"cmds":[
    {{"type":"local","name":"a","in":{{"x":{n}}},"out":{{"y":{n}}},{p}}},
    {{"type":"local","name":"b","in":{{"x":{n}}},"out":{{"y":{n}}},{p}}}],
    "cons":[{{"src":[0,"y"],"dest":[1,"x"]}},{{"src":[1,"y"],"dest":[0,"x"]}},{{"src":[1,"y"],"dest":[-2,"r"]}}]}}"#, n = N, p = P)
}

#[test]
fn flow_errors() {
  flowlang::init("data");
  std::panic::set_hook(Box::new(|_| {})); // the panics the flows provoke are caught; keep their noise out
  let loud = |m: String| -> ! { let _ = std::panic::take_hook(); panic!("{}", m) };
  let check = |ok: bool, m: String| if !ok { loud(m) };

  // 1. An op failing in the third iteration of a list-mode local.
  let ok = run(&sums_body(), r#"{"xs":[[1],[2,3],[4]]}"#);
  check(ok.as_ref().map(|o| o.to_string()).unwrap_or_default() == r#"{"r":[[13],[14,15],[16]]}"#, format!("sums: {:?}", ok.map(|o| o.to_string())));
  let e = failure(&sums_body(), r#"{"xs":[[1],[2,3],"x"]}"#);
  check(e.code == "panic", format!("code: {}", e));
  check(e.at.len() == 2, format!("two sites: {:?}", e.at));
  let (outer, inner) = (&e.at[0], &e.at[1]);
  check((outer.case, outer.op, outer.iter, outer.op_type.as_str(), outer.op_name.as_str()) == (0, Some(0), Some(2), "local", "sums"), format!("outer site: {:?}", outer));
  check((inner.case, inner.op, inner.iter, inner.op_type.as_str(), inner.op_name.as_str()) == (0, Some(1), None, "primitive", "+"), format!("inner site: {:?}", inner));
  check(e.msg == "op 1: list input 'b' is not an array", format!("msg (the stepper's text): {}", e.msg));
  let want = "flow failed (panic) at op 0 local \"sums\" (iteration 2) > op 1 primitive \"+\": list input 'b' is not an array";
  check(e.to_string() == want, format!("one line:\n  got:  {}\n  want: {}", e, want));

  // 2. A fail rule re-raising a nested fail rule: the cause says why.
  let ok = run(&check_body(), r#"{"v":"ok"}"#);
  check(ok.is_ok(), format!("check ok: {:?}", ok.err()));
  let e = failure(&check_body(), r#"{"v":"bad"}"#);
  let want = "flow failed (fail) at op 0 local \"check\": its fail rule fired; because flow failed (fail) at op 0 local \"check\" > op 0 match \"ok\": its fail rule fired";
  check(e.to_string() == want, format!("cause chain:\n  got:  {}\n  want: {}", e, want));
  let cause = e.cause.as_ref().expect("a cause");
  check(cause.at.len() == 2 && cause.at[1].op_type == "match" && cause.cause.is_none(), format!("cause: {:?}", cause));

  // Three levels: each cause sits inside the ops above it, with the iteration that failed.
  let ok = run(&outer_body(), r#"{"vs":["ok","ok"]}"#);
  check(ok.is_ok(), format!("outer ok: {:?}", ok.err()));
  let e = failure(&outer_body(), r#"{"vs":["ok","bad","ok"]}"#);
  let want = concat!(
    "flow failed (fail) at op 0 local \"outer\": its fail rule fired",
    "; because flow failed (fail) at op 0 local \"outer\" (iteration 1) > op 0 local \"check\": its fail rule fired",
    "; because flow failed (fail) at op 0 local \"outer\" (iteration 1) > op 0 local \"check\" > op 0 match \"ok\": its fail rule fired");
  check(e.to_string() == want, format!("three levels:\n  got:  {}\n  want: {}", e, want));

  // 3. A stuck case names what never fired and what never ran.
  let e = failure(&stuck_body(), "{}");
  check(e.code == "stall", format!("code: {}", e));
  check(e.at.len() == 1 && e.at[0].op.is_none(), format!("a stall belongs to the case: {:?}", e.at));
  check(e.msg == "the case is stuck: wires [0, 1, 2] never fired, and ops [0, 1] never ran (a cycle of wires and synchros, I-2)", format!("stall msg: {}", e.msg));

  // 4. The envelope callers hand back.
  let env = CodeException::Fail(e.clone()).to_envelope();
  check(env.get_string("status") == "err" && env.get_string("msg") == e.to_string(), format!("envelope: {}", env.to_string()));
  let fe = env.get_object("flow_error");
  check(fe.get_string("code") == "stall" && fe.get_array("at").len() == 1 && fe.get_property("cause").is_null(), format!("flow_error: {}", fe.to_string()));

  // 5. Arguments that do not fit the declared types (test_add: a and b are Integers).
  let add = Command::lookup("testflow", "testflow", "test_add");
  let mut good = DataObject::from_string(r#"{"a":"5","b":2}"#);
  check(add.try_cast_params(good.clone()).is_ok() && good.get_int("a") == 5, format!("cast: {}", good.to_string()));
  good.put_string("b", "x7");
  let m = add.try_cast_params(good.clone()).err().unwrap_or_default();
  check(m == "parameter 'b' of testflow:test_add must be an Integer, not \"x7\"", format!("cast message: {}", m));
  let p = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| add.cast_params(good.clone()))).err().map(|p| p.downcast_ref::<String>().cloned().unwrap_or_default());
  check(p.as_deref() == Some(m.as_str()), format!("cast_params panics with the same message: {:?}", p));

  // 6. execute_command (a primitive and a Rust command) reports the flow's failure in its envelope.
  let mut call = DataObject::new();
  call.put_string("lib", "testflow"); call.put_string("ctl", "testflow"); call.put_string("cmd", "test_lists");
  call.put_object("params", DataObject::from_string(r#"{"a":"not a list"}"#));
  let r = flowlang::flowlang::system::execute_command::execute(call).get_object("a");
  check(r.get_string("status") == "err" && r.get_string("msg").contains("at op 0 primitive \"+\": list input 'a' is not an array"), format!("execute_command: {}", r.to_string()));

  let _ = std::panic::take_hook();

  // 7. The CLI: the envelope on stdout (and nothing else), exit status 1.
  let flow = |args: &[&str], stdin: &str| {
    let mut c = Process::new(env!("CARGO_BIN_EXE_flow")).args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("flow runs");
    c.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
    let o = c.wait_with_output().unwrap();
    (o.status.code(), String::from_utf8_lossy(&o.stdout).to_string())
  };
  let (code, out) = flow(&["testflow", "testflow", "test_lists"], r#"{"a":"x"}"#);
  assert_eq!(code, Some(1), "a failed flow exits 1: {}", out);
  assert_eq!(out.lines().count(), 1, "stdout is the envelope alone: {}", out);
  let env = DataObject::from_string(out.trim());
  assert_eq!(env.get_string("status"), "err");
  assert_eq!(env.get_object("flow_error").get_string("code"), "panic");
  let (code, out) = flow(&["testflow", "testflow", "no_such_command"], "{}");
  assert_eq!(code, Some(1), "an unknown command exits 1: {}", out);
  let env = DataObject::from_string(out.trim());
  assert_eq!((env.get_string("status"), env.get_string("msg")), ("err".to_string(), "no command testflow:testflow:no_such_command: no such command".to_string()));
  let (code, out) = flow(&["testflow", "testflow", "test_add"], r#"{"a":300,"b":120}"#);
  assert_eq!((code, out.trim()), (Some(0), r#"{"a":"300+120=420"}"#));

  // 8. MCP: the tool call is an error result naming the failure; stdout stays JSON-RPC.
  let mut c = Process::new(env!("CARGO_BIN_EXE_flowmcp")).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().expect("flowmcp runs");
  let req = concat!(
    r#"{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"t","version":"0"}},"id":1}"#, "\n",
    r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"testflow-testflow-test_lists","arguments":{"a":"x"}},"id":2}"#, "\n");
  c.stdin.take().unwrap().write_all(req.as_bytes()).unwrap();
  let o = c.wait_with_output().unwrap();
  let out = String::from_utf8_lossy(&o.stdout).to_string();
  for l in out.lines() { assert!(l.starts_with('{'), "stdout carries only JSON-RPC: {:?}", l); }
  let reply = DataObject::from_string(out.lines().last().expect("a reply"));
  let res = reply.get_object("result");
  assert!(res.get_boolean("isError"), "an error result: {}", reply.to_string());
  let text = res.get_array("content").get_object(0).get_string("text");
  assert!(text.contains("flow failed (panic) at op 0 primitive \"+\": list input 'a' is not an array"), "the failure: {}", text);
}
