//! Flow correctness R1: nested outcome semantics and frame propagation.
//!
//! The stepper is the single source of truth for flow execution. A nested (local) op follows the
//! language's outcome contract (docs/flow-language.md 1.5.4 Frames / early exit), which matches
//! pre-stepper `Code::execute` (verified against 0.3.39): a child `terminate` is SWALLOWED - the
//! enclosing op completes with the child's partial `out` and success flag TRUE; a child `fail` is
//! CAUGHT as the op's FALSE condition (empty result, b=false); a child that runs out of work is a
//! normal (ok) completion; only a child `next`-with-no-next-case (I-4) is a fatal ERROR. Before
//! this fix the stepper had no arm that stepped a NESTED closing frame, so a nested I-4 and a
//! nested terminate both looped until the step budget (R1 reproductions (a) and (b)). The tests
//! below FAIL on the pre-fix stepper where the behavior differs.

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

// Build a JSON string literal from a raw string: json_escape is deliberately minimal (only '\' and
// '"' appear in these fixtures' raw strings).
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
// (a) a child local whose ONLY op fires `next` in the child's last (only) case: I-4, nested.
fn child_i4() -> String {
  let go = format!("{{\"condition\":{{\"rule\":\"next\",\"value\":true}},\"in\":{{}},\"name\":\"go\",\"out\":{{}},\"pos\":{{\"x\":0,\"y\":0,\"z\":0}},\"type\":\"local\",\"width\":1.5}}");
  case(&[go], &[], "", "", None)
}
// (b) a child local whose only op terminates on v=="stop". The parent wires a constant to out.r;
// the SWEEP runs that constant before the child finishes, so the delivered-key assertion is the
// ORDER-FREE one: out.r is delivered either way. The real (b) defects were the budget LOOP (now a
// prompt leave) and the lost `terminate` outcome; the child-delivered partial out is asserted by
// the terminate-out test below.
fn nested_terminate_body(after_name: &str) -> String {
  let child = {
    let m = op_match(0, "stop", &["a"], &[], Some(("terminate", true)));
    case(&[m], &[con(-1, "v", 0, "a")], &inp_decl(&["v"]), "", None)
  };
  let l = op_local(0, "child", &["v"], &["r"], None, &child);
  let after = op_const(1, after_name, &["c"]);
  let cons = [con(-1, "v", 0, "v"), con(1, "c", -2, "r")];
  case(&[l, after], &cons, &inp_decl(&["v"]), &outp_decl(&["r"]), None)
}
// nested terminate delivers the child's PARTIAL out: the child delivers q before terminating, the
// parent wires child.q to the return bar. After the fix the op completes with that partial out.
fn nested_terminate_out_body() -> String {
  let child = {
    let k = op_const(0, "kept", &["q"]);           // runs first, delivers q
    let m = op_match(1, "stop", &["a"], &[], Some(("terminate", true)));
    // k.q -> return bar q ; v -> m.a
    let cons = [con(0, "q", -2, "q"), con(-1, "v", 1, "a")];
    case(&[k, m], &cons, &inp_decl(&["v"]), &outp_decl(&["q"]), None)
  };
  let l = op_local(0, "child", &["v"], &["q"], None, &child);
  let cons = [con(-1, "v", 0, "v"), con(0, "q", -2, "r")];
  case(&[l], &cons, &inp_decl(&["v"]), &outp_decl(&["r"]), None)
}
// (c) nested fail is caught as the op's FALSE condition: a child that fails on v=="stop", with a
// terminate rule on the op's FALSE condition. The fail -> b=false -> the terminate fires -> the run
// terminates. v!="stop": no fail, b=true, the false-condition rule does not fire -> ok.
fn nested_fail_body() -> String {
  let child = {
    let m = op_match(0, "stop", &["a"], &[], Some(("fail", true)));
    case(&[m], &[con(-1, "v", 0, "a")], &inp_decl(&["v"]), "", None)
  };
  let l = op_local(0, "child", &["v"], &[], Some(("terminate", false)), &child);
  case(&[l], &[con(-1, "v", 0, "v")], &inp_decl(&["v"]), "", None)
}
// (d) an empty child flow is a normal completion, promptly; a downstream constant proves the parent
// continued after it (no invented I-4, no stall).
fn empty_nested_body() -> String {
  let child = case(&[], &[], "", "", None);
  let l = op_local(0, "child", &[], &[], None, &child);
  let k = op_const(1, "kept", &["c"]);
  let cons = [con(1, "c", -2, "r")];
  case(&[l, k], &cons, "", &outp_decl(&["r"]), None)
}
// root terminate on the FIRST case with a next case still pending: partial output, normal ok; the
// next case must NOT run.
fn root_terminate_body() -> String {
  let m = op_match(0, "stop", &["a"], &[], Some(("terminate", true)));
  let first = case(&[m], &[con(-1, "v", 0, "a")], &inp_decl(&["v"]), "", None);
  let c = op_const(0, "AFTER_NESTED_TERMINATE", &["c"]);
  let second = case(&[c], &[con(0, "c", -2, "r")], "", &outp_decl(&["r"]), None);
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
// a guaranteed stall: two localdata-less local ops feed each other, so neither ever runs and the
// return-bar wire off op0 never fires -> the case ends with an unfired wire.
fn root_stall_body() -> String {
  let a = format!("{{\"in\":{{\"a\":{{\"mode\":\"in\",\"type\":\"Any\",\"x\":0}}}},\"name\":\"needs_a\",\"out\":{{\"y\":{{\"mode\":\"out\",\"type\":\"Any\",\"x\":0}}}},\"pos\":{{\"x\":0,\"y\":0,\"z\":0}},\"type\":\"local\",\"width\":1.5}}");
  let b = format!("{{\"in\":{{\"a\":{{\"mode\":\"in\",\"type\":\"Any\",\"x\":0}}}},\"name\":\"feeder\",\"out\":{{\"y\":{{\"mode\":\"out\",\"type\":\"Any\",\"x\":0}}}},\"pos\":{{\"x\":1,\"y\":0,\"z\":0}},\"type\":\"local\",\"width\":1.5}}");
  let cons = [con(1, "y", 1, "a"), con(1, "y", 0, "a"), con(0, "y", -2, "r")];
  case(&[a, b], &cons, "", &outp_decl(&["r"]), None)
}
// nested stall: the guaranteed-stall case one frame down.
fn nested_stall_body() -> String {
  let child = root_stall_body();
  let l = op_local(0, "child", &[], &[], None, &child);
  case(&[l], &[], "", "", None)
}
// a primitive op fed the wrong-typed inputs (std primitives self-catch into an err envelope).
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
  assert_eq!(r.event.get_string("type"), "error");
  assert_eq!(ec(&r), "I-4", "the terminal event must be the real I-4, not a budget error: {:?}", r.event);
  assert!(r.steps < 100, "no residual stepping loop: steps={} must be < the 100 budget", r.steps);
  assert_eq!(run_body(&body, "{}").err(), Some(CodeException::Fail));
}

// (b) nested terminate is SWALLOWED: the run completes ok and promptly (no budget loop). The
// constant wired to out.r runs under the sweep either way (order-free), so the discriminating
// assertions are the prompt completion and the delivered partial out (next test).
#[test]
fn r1b_nested_terminate_completes_promptly_not_budget() {
  init();
  let body = nested_terminate_body("AFTER_NESTED_TERMINATE");
  let r = report(&body, "{\"v\":\"stop\"}", 100);
  assert_eq!(r.event.get_string("type"), "done", "a nested terminate leaves the op done: {:?}", r.event);
  assert!(r.steps < 100, "no residual stepping loop: steps={}", r.steps);
  let out = run_body(&body, "{\"v\":\"stop\"}").expect("a nested terminate is a normal completion");
  assert_eq!(out.get_string("r"), "AFTER_NESTED_TERMINATE");
  let r2 = run_body(&body, "{\"v\":\"go\"}").expect("v!=stop never fires terminate");
  assert_eq!(r2.get_string("r"), "AFTER_NESTED_TERMINATE");
}

// (b2) the child's PARTIAL out survives the swallow: a key the child delivered before terminating
// reaches the parent's return bar.
#[test]
fn r1b2_nested_terminate_delivers_partial_out() {
  init();
  let body = nested_terminate_out_body();
  let out = run_body(&body, "{\"v\":\"stop\"}").expect("nested terminate completes ok");
  assert_eq!(out.get_string("r"), "kept", "the child's partial out reaches the parent");
}

// (b3) the DEBUGGER's path: stepping the frame stack one step at a time (dev.flowstep's step)
// runs the child to its closing event while the parent's constant is still pending, then must LEAVE
// the child and finish the run - not loop. Pre-fix there was no arm for a nested closing frame, so
// the next step after the child's terminate stalled into the budget. This is reproduction (b) as
// the live debugger hits it (`st::run(.., 100)` -> a budget error with the child stuck ended).
#[test]
fn r1b3_stepping_nested_terminate_leaves_then_finishes() {
  init();
  let body = nested_terminate_body("AFTER_NESTED_TERMINATE");
  let mut frames = ndata::dataarray::DataArray::new();
  frames.push_object(stepper::new_frame(DataObject::from_string("{\"v\":\"stop\"}"), "root", Case::new(&body).to_data(), ndata::dataarray::DataArray::new()));
  let mut saw_terminate = false;
  let mut saw_leave = false;
  let mut steps = 0;
  loop {
    if frames.len() == 0 || frames.get_object(0).get_string("phase") == "ended" { break; }
    let e = stepper::step_stack(&mut frames, &DataObject::new());
    steps += 1;
    let t = if e.has("type") { e.get_string("type") } else { String::new() };
    if t == "terminate" { saw_terminate = true; }
    if t == "leave" { saw_leave = true; }
    if t == "error" || t == "stall" { panic!("stepping a nested terminate must not error/stall: {:?}", e); }
    if steps > 60 { panic!("stepping a nested terminate looped (no leave / no done): {} steps", steps); }
  }
  assert!(saw_terminate, "the child's terminate event is observed");
  assert!(saw_leave, "the nested closing frame must LEAVE (the pre-fix stepper never left it)");
  assert!(steps < 60, "the run finished in {} steps", steps);
}

// (c) root terminate is normal completion with partial output, even with a next case pending.
#[test]
fn r1c_root_terminate_is_partial_ok() {
  init();
  let body = root_terminate_body();
  let out = run_body(&body, "{\"v\":\"stop\"}").expect("root terminate returns Ok(partial out)");
  assert!(!out.has("r"), "root terminate must not evaluate the next case (out={:?})", out);
  let r = report(&body, "{\"v\":\"stop\"}", 0);
  assert_eq!(r.outcome, "terminate");
}

// (c2) nested fail is caught as the op's FALSE condition (not propagated, not root-fatal).
#[test]
fn r1c2_nested_fail_is_catchable_as_false_condition() {
  init();
  let body = nested_fail_body();
  let r = report(&body, "{\"v\":\"stop\"}", 0);
  assert_eq!(r.outcome, "terminate", "the op's false-condition terminate fires: {:?}", r.event);
  assert!(run_body(&body, "{\"v\":\"go\"}").is_ok(), "v!=stop: no fail, normal completion");
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
  assert_eq!(run_body(&body, "{\"v\":\"stop\"}").err(), Some(CodeException::Fail));
}

// nested stall ends the run promptly (no budget burn).
#[test]
fn r1_nested_stall_is_a_prompt_error() {
  init();
  let r = report(&nested_stall_body(), "{}", 100);
  assert_eq!(r.event.get_string("type"), "stall", "a nested stall ends on a stall event, not a budget error: {:?}", r.event);
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

// a primitive op fed nulls self-catches into an err envelope: the op completes, the run ends
// promptly (the interpreter neither unwinds nor hangs on a bad primitive call).
#[test]
fn r1_primitive_bad_args_completes_promptly() {
  init();
  let r = report(&prim_bad_args_body(), "{}", 100);
  assert!(r.steps < 100, "a bad primitive call must not burn the budget: steps={}", r.steps);
  assert!(run_body(&prim_bad_args_body(), "{}").is_ok(), "a self-catching primitive returns Ok");
}

// one frame down: the child's (self-caught) completion must propagate back to the parent, not stall.
#[test]
fn r1_nested_primitive_completes_promptly() {
  init();
  let r = report(&nested_prim_body(), "{}", 100);
  assert!(r.steps < 100, "a nested primitive must not burn the budget: steps={}", r.steps);
  assert!(run_body(&nested_prim_body(), "{}").is_ok());
}
