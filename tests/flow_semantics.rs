//! Four sharp edges of the flow language, fixed in the stepper (the definition) and the fast
//! path together. Each case runs through both `stepper::run_report` and `Code::execute`, and
//! both must give the stated result. No flow that ran deterministically before changes its
//! result: each fix only touches flows that stalled, failed or mapped at random.
//!
//! 1. An op input is awaited only when a wire targets THAT input. Before, an input counted as
//!    awaited when its NAME was any wire's dest name anywhere in the case (the return bar
//!    included), so an unwired input sharing a name with a wired terminal waited forever.
//!    Now it is pre-delivered as null, like any other unwired input.
//! 2. A command op's outputs map onto the callee's outputs by name. Only when both have
//!    exactly one output do they map by position, whatever their names, as before. Before,
//!    every mapping was by position over unordered keys, so a multi-output callee's values
//!    landed on the op's outputs at random.
//! 3. A string constant still has its escapes read (`a\nb` is a newline). Text that is not a
//!    valid escaped string (`say "hi"`, a lone `\`) is taken as written, instead of failing as
//!    "bad string literal".
//! 4. An op of an unknown type is an error naming the type. Before, it silently yielded `{}`.
//!
//! One #[test] on purpose: the store and ndata globals initialize once per process.

use flowlang::case::Case;
use flowlang::code::{Code, CodeException};
use flowlang::stepper;
use ndata::dataobject::DataObject;

const N: &str = r#"{"mode":"regular","type":"Any","x":0}"#;
const P: &str = r#""pos":{"x":0,"y":0,"z":0},"width":1"#;
/// testflow's test_loop: a flow command with two outputs, a (an int) and b (a list).
const TEST_LOOP: &str = "testflow:testflow:izmuzm18037d796f1i467";

/// The stepper's run and Code::execute's, as `ok <out json>` or `fail <code>: <msg>`.
fn both(body: &str, args: &str) -> (String, String) {
  let case = Case::from_data(DataObject::from_string(body));
  let s = stepper::run_report(case.to_data(), DataObject::from_string(args), 0);
  let st = if s.outcome == "fail" {
    let e = &s.event;
    let t = e.get_string("type");
    if t == "error" { format!("fail {}: {}", e.get_string("code"), e.get_string("msg")) } else { format!("fail {}", t) }
  } else { format!("ok {}", canon(&s.out)) };
  let c = match Code::new(case).execute(DataObject::from_string(args)) {
    Ok(o) => format!("ok {}", canon(&o)),
    Err(CodeException::Fail(e)) => if e.code == "stall" || e.code == "fail" { format!("fail {}", e.code) } else { format!("fail {}: {}", e.code, e.msg) },
    Err(other) => format!("{:?}", other),
  };
  (st, c)
}
fn canon(o: &DataObject) -> String {
  let mut ks = o.clone().keys(); ks.sort();
  ks.iter().map(|k| format!("{}={}", k, ndata::data::Data::as_string(o.get_property(k)))).collect::<Vec<_>>().join(",")
}
fn expect(label: &str, body: &str, args: &str, want: &str) {
  let (s, c) = both(body, args);
  assert_eq!(s, want, "{}: the stepper", label);
  assert_eq!(c, want, "{}: Code::execute", label);
}

#[test]
fn flow_semantics() {
  flowlang::init("data");
  std::panic::set_hook(Box::new(|_| {}));
  let r = std::panic::catch_unwind(|| {
    // 1. Awaited means wired to that input. Op 1's input x has no wire, but a wire into the
    //    return bar is also named x: before, op 1 waited forever and the case stalled.
    let await_body = format!(r#"{{"input":{{}},"output":{{"x":{n},"y":{n}}},"cmds":[
      {{"type":"constant","ctype":"int","name":"1","in":{{}},"out":{{"a":{n}}},{p}}},
      {{"type":"local","name":"unwired","in":{{"x":{n}}},"out":{{"y":{n}}},{p}}}],
      "cons":[{{"src":[0,"a"],"dest":[-2,"x"]}},{{"src":[1,"y"],"dest":[-2,"y"]}}]}}"#, n = N, p = P);
    expect("unwired input sharing a wired name", &await_body, "{}", "ok x=1,y=null");

    // 2. A two-output callee maps by name, every run.
    let call = |outs: &str| format!(r#"{{"input":{{"a":{n}}},"output":{{"aa":{n},"bb":{n}}},"cmds":[
      {{"type":"command","cmd":"{c}","name":"test_loop","in":{{"a":{n}}},"out":{outs},{p}}}],
      "cons":[{{"src":[-1,"a"],"dest":[0,"a"]}},{{"src":[0,"a"],"dest":[-2,"aa"]}},{{"src":[0,"b"],"dest":[-2,"bb"]}}]}}"#,
      n = N, p = P, c = TEST_LOOP, outs = outs);
    let ab = call(&format!(r#"{{"b":{n},"a":{n}}}"#, n = N));
    for _ in 0..20 { expect("two outputs by name", &ab, r#"{"a":2}"#, "ok aa=4,bb=[3,4]"); }
    // an op output the callee does not have is an error naming it
    let xb = format!(r#"{{"input":{{"a":{n}}},"output":{{"bb":{n}}},"cmds":[
      {{"type":"command","cmd":"{c}","name":"test_loop","in":{{"a":{n}}},"out":{{"x":{n},"b":{n}}},{p}}}],
      "cons":[{{"src":[-1,"a"],"dest":[0,"a"]}},{{"src":[0,"b"],"dest":[-2,"bb"]}}]}}"#, n = N, p = P, c = TEST_LOOP);
    expect("an output the callee lacks", &xb, r#"{"a":2}"#,
      "fail absent-output: op 0: command op output 'x' names no output of the callee (its outputs: a, b)");
    // one output to one output still maps by position, whatever the names (test_command's shape)
    let one = format!(r#"{{"input":{{"a":{n}}},"output":{{"r":{n}}},"cmds":[
      {{"type":"command","cmd":"testflow:testflow:zkuwhn1802d57cb8ak1c","name":"test_add","in":{{"a":{n},"b":{n}}},"out":{{"result":{n}}},{p}}}],
      "cons":[{{"src":[-1,"a"],"dest":[0,"a"]}},{{"src":[-1,"a"],"dest":[0,"b"]}},{{"src":[0,"result"],"dest":[-2,"r"]}}]}}"#, n = N, p = P);
    expect("one to one by position", &one, r#"{"a":5}"#, "ok r=5+5=10");

    // 3. String constants: escapes are read; text that is not a valid escaped string is kept.
    let constant = |lit: &str| format!(r#"{{"input":{{}},"output":{{"s":{n}}},"cmds":[
      {{"type":"constant","ctype":"string","name":{lit},"in":{{}},"out":{{"a":{n}}},{p}}}],
      "cons":[{{"src":[0,"a"],"dest":[-2,"s"]}}]}}"#, n = N, p = P, lit = serde_like(lit));
    expect("an escape", &constant("a\\nb"), "{}", "ok s=a\nb");
    expect("an escaped quote", &constant("say \\\"hi\\\""), "{}", "ok s=say \"hi\"");
    expect("a bare quote", &constant("say \"hi\""), "{}", "ok s=say \"hi\"");
    expect("a lone backslash", &constant("back\\"), "{}", "ok s=back\\");

    // 4. An unknown op type is an error.
    let unknown = format!(r#"{{"input":{{}},"output":{{}},"cmds":[
      {{"type":"mystery","name":"m","in":{{}},"out":{{}},{p}}}],"cons":[]}}"#, p = P);
    expect("an unknown op type", &unknown, "{}", "fail panic: op 0: op type 'mystery' is unknown");
  });
  let _ = std::panic::take_hook();
  if let Err(p) = r {
    let m = p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
    panic!("{}", m);
  }
}

/// `s` as a JSON string literal (the constant's name as stored).
fn serde_like(s: &str) -> String {
  let mut o = String::from("\"");
  for c in s.chars() {
    match c { '"' => o.push_str("\\\""), '\\' => o.push_str("\\\\"), '\n' => o.push_str("\\n"), c => o.push(c) }
  }
  o.push('"');
  o
}
