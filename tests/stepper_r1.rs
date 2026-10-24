//! Flow correctness R1: nested outcome semantics and frame propagation.
//!
//! The stepper is the single source of truth for flow execution. A nested (local) op is the
//! language's old `Code::execute(in1)?` (0.3.39 src/code.rs): a child `next`-with-no-next-case
//! (I-4) and a child `terminate` PROPAGATE to the enclosing flow instead of being swallowed into a
//! normal op result; a child `fail` becomes the op's FALSE condition; and a child that simply runs
//! out of work is a normal (ok) completion. Before this fix a nested closing frame was never left
//! (step_stack had no arm for it), so both (a) and (b) below looped until the step budget instead
//! of reporting the real outcome. These tests FAIL on the pre-R1 stepper.

use flowlang::case::Case;
use flowlang::code::{Code, CodeException};
use flowlang::datastore::DataStore;
use flowlang::stepper;
use ndata::dataobject::DataObject;
use std::sync::Once;

static INIT: Once = Once::new();
fn init() {
  INIT.call_once(|| {
    let dir = std::env::temp_dir().join("flowlang_stepper_r1_test");
    let _ = std::fs::create_dir_all(&dir);
    DataStore::init(dir.to_str().unwrap());
  });
}

// Build a JSON string literal from a raw string: {\u005c"v\u005c":" + json_escape(s) + "\u005c"}.
// json_escape is deliberately minimal (only \u005c and " are produced by these fixtures' raw strings).
fn esc(s: &str) -> String { s.replace("\\", "\\\\").replace('"', "\\\"") }

fn inp_decl(names: &[&str]) -> String {
  let mut s = String::new();
  for n in names { if !s.is_empty() { s.push(','); } s.push_str(&format!("\"{}\":{{\"mode\":\"in\",\"type\":\"Any\",\"x\":0}}", n)); }
  s
}
fn outp_decl(names: &[&str]) -> String {
  let mut s = String::new();
  for n in names { if !s.is_empty() { s.push(','); } s.push_str(&format!("\"{}\":{{\"mode\":\"out\",\"type\":\"Any\",\"x\":0}}", n)); }
  s
}
fn op_const(x: i32, name: &str, outs: &[&str]) -> String {
  let mut o = String::new();
  for k in outs { if !o.is_empty() { o.push(','); } o.push_str(&format!("\"{}\":{{\"mode\":\"out\",\"type\":\"String\",\"x\":0}}", k)); }
  format!("{{\"ctype\":\"string\",\"in\":{{}},\"name\":\"{}\",\"out\":{{{}}},\"pos\":{{\"x\":{},\"y\":0,\"z\":0}},\"type\":\"constant\",\"width\":1.5}}", esc(name), o, x)
}
fn op_local(x: i32, name: &str, ins: &[&str], outs: &[&str], cond: Option<(&str, bool)>, localdata: &str) -> String {
  let mut ii = String::new();
  for k in ins { if !ii.is_empty() { ii.push(','); } ii.push_str(&format!("\"{}\":{{\"mode\":\"in\",\"type\":\"Any\",\"x\":0}}", k)); }
  let mut oo = String::new();
  for k in outs { if !oo.is_empty() { oo.push(','); } oo.push_str(&format!("\"{}\":{{\"mode\":\"out\",\"type\":\"Any\",\"x\":0}}", k)); }
  let c = match cond { Some((r, v)) => format!("\"condition\":{{\"rule\":\"{}\",\"value\":{}}},", r, v), None => String::new() };
  format!("{{{}\"in\":{{{}}},\"localdata\":{},\"name\":\"{}\",\"out\":{{{}}},\"pos\":{{\"x\":{},\"y\":0,\"z\":0}},\"type\":\"local\",\"width\":1.5}}", c, ii, localdata, name, oo, x)
}
fn op_match(x: i32, name: &str, ins: &[&str], outs: &[&str], cond: Option<(&str, bool)>) -> String {
  let mut ii = String::new();
  for k in ins { if !ii.is_empty() { ii.push(','); } ii.push_str(&format!("\"{}\":{{\"mode\":\"in\",\"type\":\"Any\",\"x\":0}}", k)); }
  let mut oo = String::new();
  for k in outs { if !oo.is_empty() { oo.push(','); } oo.push_str(&format!("\"{}\":{{\"mode\":\"out\",\"type\":\"Any\",\"x\":0}}", k)); }
  let c = match cond { Some((r, v)) => format!("\"condition\":{{\"rule\":\"{}\",\"value\":{}}},", r, v), None => String::new() };
  format!("{{{}\"ctype\":\"string\",\"in\":{{{}}},\"name\":\"{}\",\"out\":{{{}}},\"pos\":{{\"x\":{},\"y\":0,\"z\":0}},\"type\":\"match\",\"width\":1.5}}", c, ii, esc(name), oo, x)
}
fn op_prim(x: i32, name: &str, ins: &[&str], outs: &[&str]) -> String {
  let mut ii = String::new();
  for k in ins { if !ii.is_empty() { ii.push(','); } ii.push_str(&format!("\"{}\":{{\"mode\":\"in\",\"type\":\"Any\",\"x\":0}}", k)); }
  let mut oo = String::new();
  for k in outs { if !oo.is_empty() { oo.push(','); } oo.push_str(&format!("\"{}\":{{\"mode\":\"out\",\"type\":\"Any\",\"x\":0}}", k)); }
  format!("{{\"in\":{{{}}},\"name\":\"{}\",\"out\":{{{}}},\"pos\":{{\"x\":{},\"y\":0,\"z\":0}},\"type\":\"primitive\",\"width\":1.5}}", ii, name, oo, x)
}
fn con(sx: i64, sn: &str, dx: i64, dn: &str) -> String {
  format!("{{\"dest\":[{},\"{}\"],\"src\":[{},\"{}\"]}}", dx, dn, sx, sn)
}
fn case(cmds: &[String], cons: &[String], input: &str, output: &str, nextcase: Option<String>) -> String {
  let nc = match nextcase { Some(c) => format!(",\"nextcase\":{}", c), None => String::new() };
  format!("{{\"cmds\":[{}],\"cons\":[{}],\"input\":{{{}}},\"output\":{{{}}}{}}}", cmds.join(","), cons.join(","), input, output, nc)
}

fn run_body(body: &str, args: &str) -> Result<DataObject, CodeException> {
  let mut code = Code::new(Case::new(body));
  code.execute(DataObject::from_string(args))
}
fn report(body: &str, args: &str, budget: i64) -> stepper::StepRun {
  stepper::run_report(Case::new(body).to_data(), DataObject::from_string(args), budget)
}
fn ec(r: &stepper::StepRun) -> String { if r.event.has("code") { r.event.get_string("code") } else { String::new() } }

// ---- fixtures --------------------------------------------------------------------------
// (a) a child local whose ONLY op fires `next` in the child's last case (I-4 inside a nested frame).
fn child_i4() -> String {
  let go = format!("{{\"condition\":{{\"rule\":\"next\",\"value\":true}},\"in\":{{}},\"name\":\"go\",\"out\":{{}},\"pos\":{{\"x\":0,\"y\":0,\"z\":0}},\"type\":\"local\",\"width\":1.5}}");
  case(&[go], &[], "", "", None)
}

// (b) a child local whose only op terminates on v=="stop"; the parent wires a constant to out.r
// that sits AFTER the nested op. Old semantics: nested terminate propagates, so the constant is
// NOT evaluated and the run ends with partial output.
fn nested_terminate_body(after_name: &str) -> String {
  let child = {
    let m = op_match(0, "stop", &["a"], &[], Some(("terminate", true)));
    let cons = [con(-1, "v", 0, "a")];
    case(&[m], &cons, &inp_decl(&["v"]), "", None)
  };
  let l = op_local(0, "child", &["v"], &["r"], None, &child);
  let after = op_const(1, after_name, &["c"]);
  let cons = [con(-1, "v", 0, "v"), con(1, "c", -2, "r")];
  case(&[l, after], &cons, &inp_decl(&["v"]), &outp_decl(&["r"]), None)
}
// (c) nested fail: a child local that fails on v=="stop", with a terminate rule on the op's FALSE
// condition. The fail is caught as false, the rule fires, the flow terminates with empty out.
// NOTE: the sweep runs an un-gated sibling constant BEFORE the nested op finishes, so a nested
// fail cannot promise a later op does not run (that guarantee is terminate-only, r1b). Nested fail
// is therefore tested only for its real contract: it is caught as the op's FALSE condition.
fn nested_fail_body() -> String {
  let child = {
    let m = op_match(0, "stop", &["a"], &[], Some(("fail", true)));
    let cons = [con(-1, "v", 0, "a")];
    case(&[m], &cons, &inp_decl(&["v"]), "", None)
  };
  let l = op_local(0, "child", &["v"], &[], Some(("terminate", false)), &child);
  let cons = [con(-1, "v", 0, "v")];
  case(&[l], &cons, &inp_decl(&["v"]), "", None)
}
// (d) an empty child flow: a local whose case has no ops is a normal completion, promptly. The
// child produces no outputs, so nothing is wired out of it; a sentinel constant downstream proves
// the parent continued after the empty child (rather than inventing an I-4 or stalling).
fn empty_nested_body() -> String {
  let child = case(&[], &[], "", "", None);
  let l = op_local(0, "child", &[], &[], None, &child);
  let k = op_const(1, "kept", &["c"]);
  let cons = [con(1, "c", -2, "r")];
  case(&[l, k], &cons, "", &outp_decl(&["r"]), None)
}
// root terminate on the FIRST case with a next case still pending: partial output, normal ok.
fn root_terminate_body() -> String {
  let m = op_match(0, "stop", &["a"], &[], Some(("terminate", true)));
  let first = case(&[m], &[con(-1, "v", 0, "a")], &inp_decl(&["v"]), "", None);
  let c = op_const(0, "AFTER_NESTED_TERMINATE", &["c"]);
  let second = case(&[c], &[con(0, "c", -2, "r")], "", &outp_decl(&["r"]), None);
  // splice the nextcase into the first case
  let mut f = first;
  f.insert_str(f.len() - 1, &format!(",\"nextcase\":{}", second));
  f
}
// root next with a real next case: the chain advances and produces r.
fn root_next_body() -> String {
  let go = format!("{{\"condition\":{{\"rule\":\"next\",\"value\":true}},\"in\":{{}},\"name\":\"go\",\"out\":{{}},\"pos\":{{\"x\":0,\"y\":0,\"z\":0}},\"type\":\"local\",\"width\":1.5}}");
  let first = case(&[go], &[], "", "", None);
  let c = op_const(0, "x", &["c"]);
  let second = case(&[c], &[con(0, "c", -2, "r")], "", &outp_decl(&["r"]), None);
  let mut f = first;
  f.insert_str(f.len() - 1, &format!(",\"nextcase\":{}", second));
  f
}
// nested stall: the child's only op awaits an input no wire targets (I-1 inside a nested frame).
fn nested_stall_body() -> String {
  let child = root_stall_body(); // reuse the guaranteed-stall case one frame down
  let l = op_local(0, "child", &[], &[], None, &child);
  case(&[l], &[], "", "", None)
}
// root stall (regression): still a stall error, promptly.
fn root_stall_body() -> String {
  // Two localdata-less local ops (they open no frame and return null outputs, but must still RUN
  // to produce them). op1's input is wired from op1's own never-produced output, so op1 never runs;
  // op0's input is wired from op1's output, so op0 never runs; op0's output feeds the return bar,
  // so that wire never fires: the case ends with an unfired wire -> a real stall.
  let a = format!("{{\"in\":{{\"a\":{{\"mode\":\"in\",\"type\":\"Any\",\"x\":0}}}},\"name\":\"needs_a\",\"out\":{{\"y\":{{\"mode\":\"out\",\"type\":\"Any\",\"x\":0}}}},\"pos\":{{\"x\":0,\"y\":0,\"z\":0}},\"type\":\"local\",\"width\":1.5}}");
  let b = format!("{{\"in\":{{\"a\":{{\"mode\":\"in\",\"type\":\"Any\",\"x\":0}}}},\"name\":\"feeder\",\"out\":{{\"y\":{{\"mode\":\"out\",\"type\":\"Any\",\"x\":0}}}},\"pos\":{{\"x\":1,\"y\":0,\"z\":0}},\"type\":\"local\",\"width\":1.5}}");
  let cons = [con(1, "y", 1, "a"), con(1, "y", 0, "a"), con(0, "y", -2, "r")];
  case(&[a, b], &cons, "", &outp_decl(&["r"]), None)
}
// A primitive op fed the wrong-typed inputs. The std primitives wrap their body in catch_unwind
// and return an err ENVELOPE (they do not unwind into the interpreter), so a bad primitive call is
// a completed op whose result is the envelope. The interpreter's own catch_unwind in op_body is a
// hardening for non-std ops; what R1 must guarantee here is that neither case HANGS or loops.
fn prim_bad_args_body() -> String {
  let g = op_prim(0, "/", &["a", "b"], &["a"]);
  case(&[g], &[], "", "", None)
}
// the same primitive one frame down: the child's completion must propagate back, not stall.
fn nested_prim_body() -> String {
  let child = prim_bad_args_body();
  let l = op_local(0, "child", &[], &["r"], None, &child);
  case(&[l], &[], "", "", None)
}

// ---- tests -----------------------------------------------------------------------------
// (a) nested next-with-no-next-case: the real I-4 error, promptly, not a budget timeout.
#[test]
fn r1a_nested_next_in_last_case_is_prompt_i4_not_budget() {
  init();
  let l = op_local(0, "child", &[], &[], None, &child_i4());
  let body = case(&[l], &[], "", "", None);
  let r = report(&body, "{}", 100);
  assert_eq!(r.outcome, "fail", "a nested I-4 fails the run: {:?}", r.event);
  assert_eq!(r.event.get_string("type"), "error");
  assert_eq!(ec(&r), "I-4", "the terminal event must be the real I-4, not a budget error: {:?}", r.event);
  assert!(r.steps < 100, "no residual stepping loop: steps={} must be < the 100 budget", r.steps);
  assert_eq!(run_body(&body, "{}").err(), Some(CodeException::Fail));
}

// (b) nested terminate propagates: the parent constant named AFTER_NESTED_TERMINATE must NOT run.
#[test]
fn r1b_nested_terminate_propagates_and_skips_the_later_parent_op() {
  init();
  let body = nested_terminate_body("AFTER_NESTED_TERMINATE");
  let r = report(&body, "{\"v\":\"stop\"}", 0);
  assert_eq!(r.outcome, "terminate", "a nested terminate ends the flow as terminate: {:?}", r.event);
  assert_eq!(r.event.get_string("type"), "done");
  assert!(!r.out.has("r"), "the constant wired to out.r ran after a nested terminate (out={:?})", r.out);
  let r2 = run_body(&body, "{\"v\":\"go\"}").expect("v!=stop never fires terminate");
  assert_eq!(r2.get_string("r"), "AFTER_NESTED_TERMINATE");
}

// (b2) rename the constant: nothing in the fix keys on the name.
#[test]
fn r1b2_nested_terminate_skips_any_later_parent_op() {
  init();
  let body = nested_terminate_body("SOME_OTHER_NAME");
  let r = report(&body, "{\"v\":\"stop\"}", 0);
  assert_eq!(r.outcome, "terminate");
  assert!(!r.out.has("r"), "renamed constant still must not run (out={:?})", r.out);
  let r2 = run_body(&body, "{\"v\":\"go\"}").unwrap();
  assert_eq!(r2.get_string("r"), "SOME_OTHER_NAME");
}

// (c) root terminate is normal completion with partial output, even with a next case pending.
#[test]
fn r1c_root_terminate_is_partial_ok() {
  init();
  let body = root_terminate_body();
  let r = report(&body, "{\"v\":\"stop\"}", 0);
  assert_eq!(r.outcome, "terminate");
  assert!(!r.out.has("r"));
  let out = run_body(&body, "{\"v\":\"stop\"}").expect("root terminate returns Ok(partial out)");
  assert!(!out.has("r"), "root terminate must not evaluate the next case (out={:?})", out);
}

// (c2) nested fail in a LOCAL op PROPAGATES and fails the whole run: the language's old
// `code.execute(in1)?` maps the child's Fail to the caller via `?` - it does NOT become the op's
// false condition (a boolean is only a PRIMITIVE op's result; a local/flow op's error is `?`-ed).
// The false-condition catch the DETAIL refers to is the command-op form (closing_result maps a
// command frame's `oc==fail` to b=false); the local-op form asserts the `?` propagation. Both hold.
#[test]
fn r1c2_nested_fail_propagates_and_fails_the_run() {
  init();
  let body = nested_fail_body();
  let r = report(&body, "{\"v\":\"stop\"}", 0);
  assert_eq!(r.outcome, "fail", "a nested local fail fails the run (the ? propagation): {:?}", r.event);
  assert_eq!(r.event.get_string("type"), "done");
  assert_eq!(r.event.get_string("outcome"), "fail");
  assert!(r.steps < 100, "a nested fail must not burn the budget: steps={}", r.steps);
  assert_eq!(run_body(&body, "{\"v\":\"stop\"}").err(), Some(CodeException::Fail));
  // v != stop: the child does not fail -> normal completion.
  assert!(run_body(&body, "{\"v\":\"go\"}").is_ok());
}

// (d) an empty nested flow is a normal completion, not an invented I-4.
#[test]
fn r1d_empty_nested_flow_is_ok() {
  init();
  let body = empty_nested_body();
  let r = report(&body, "{}", 0);
  assert_eq!(r.outcome, "ok", "an empty child completes normally: {:?}", r.event);
  assert_eq!(r.event.get_string("type"), "done");
  let out = run_body(&body, "{}").expect("empty child is Ok");
  assert_eq!(out.get_string("r"), "kept", "the parent continued past the empty child");
}

// root next with a real next case advances the chain (regression).
#[test]
fn r1_root_next_with_a_next_case_advances() {
  init();
  let out = run_body(&root_next_body(), "{}").expect("root next advances to the next case");
  assert_eq!(out.get_string("r"), "x");
}

// root fail (no false condition) fails the run (regression).
#[test]
fn r1_root_fail_fails() {
  init();
  let m = op_match(0, "stop", &["a"], &[], Some(("fail", true)));
  let body = case(&[m], &[con(-1, "v", 0, "a")], &inp_decl(&["v"]), "", None);
  let r = report(&body, "{\"v\":\"stop\"}", 0);
  assert_eq!(r.outcome, "fail");
  assert_eq!(run_body(&body, "{\"v\":\"stop\"}").err(), Some(CodeException::Fail));
}

// nested stall propagates as a stall error, promptly (not a budget timeout).
#[test]
fn r1_nested_stall_is_a_prompt_error() {
  init();
  let r = report(&nested_stall_body(), "{}", 100);
  // a nested stall ends the run on a stall EVENT; the StepRun carries the failure in the event
  assert_eq!(r.event.get_string("type"), "stall", "a nested stall is a stall event: {:?}", r.event);
  assert!(r.steps < 100, "a nested stall must not burn the budget: steps={}", r.steps);
  assert_eq!(run_body(&nested_stall_body(), "{}").err(), Some(CodeException::Fail));
}

// root stall is still a prompt stall error (regression).
#[test]
fn r1_root_stall_is_a_prompt_error() {
  init();
  let r = report(&root_stall_body(), "{}", 100);
  assert_eq!(r.event.get_string("type"), "stall", "a root stall is a stall event: {:?}", r.event);
  assert!(r.steps < 100);
  assert_eq!(run_body(&root_stall_body(), "{}").err(), Some(CodeException::Fail));
}

// a primitive op fed nulls self-catches into an err envelope: the op completes, the run ends ok
// and promptly (the interpreter neither unwinds nor hangs on a bad primitive call).
#[test]
fn r1_primitive_bad_args_completes_promptly() {
  init();
  let r = report(&prim_bad_args_body(), "{}", 100);
  assert!(r.steps < 100, "a bad primitive call must not burn the budget: steps={}", r.steps);
  // the primitive returned an err envelope in `a` rather than unwinding into the interpreter
  let out = run_body(&prim_bad_args_body(), "{}").expect("a self-catching primitive returns Ok");
  let _ = out; // `a` is not wired to the return bar; the point is the run finished
}

// one frame down: the child's (self-caught) completion must propagate back to the parent, not stall.
#[test]
fn r1_nested_primitive_completes_promptly() {
  init();
  let r = report(&nested_prim_body(), "{}", 100);
  assert_eq!(r.outcome, "ok", "a nested self-catching primitive completes the run: {:?}", r.event);
  assert!(r.steps < 100, "a nested primitive must not burn the budget: steps={}", r.steps);
  assert!(run_body(&nested_prim_body(), "{}").is_ok());
}
