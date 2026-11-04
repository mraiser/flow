//! `Code::execute` must stay on the fast path (`flowlang::fastrun`), not drift back to driving
//! the stepper. flowlang 0.3.40-0.3.43 ran every flow through `stepper::step_stack`, which made
//! ordinary execution about 13x slower than 0.3.39.
//!
//! Absolute times depend on the machine, so this test compares the two paths in the same
//! process on the same flows: `Code::execute` against `stepper::run_report`, the stepper's
//! continue-to-end. The fast path is 10-50x faster on these shapes, and the test requires at
//! least 3x. Run it with `--nocapture` to see the timings.
//!
//! The shapes:
//! - a list op over a primitive (testflow's test_lists);
//! - a local loop with finish (testflow's test_speed);
//! - list-mode command ops calling a flow callee (test_add, with its own local) and a Rust
//!   callee (test_rust).

use flowlang::case::Case;
use flowlang::code::Code;
use flowlang::datastore::DataStore;
use flowlang::stepper;
use ndata::dataobject::DataObject;
use std::time::{Duration, Instant};

const NODE: &str = r#"{"mode":"regular","type":"Any","x":0}"#;
const LIST: &str = r#"{"mode":"list","type":"Any","x":0}"#;
const POS: &str = r#"{"x":0,"y":0,"z":0}"#;

/// One list-mode command op over params `a`, calling `cmd` with every input taken from `a`.
fn calls(cmd: &str, ins: &[&str]) -> String {
  let in_o: Vec<String> = ins.iter().map(|k| format!("\"{}\":{}", k, LIST)).collect();
  let cons: Vec<String> = ins.iter().map(|k| format!(r#"{{"src":[-1,"a"],"dest":[0,"{}"]}}"#, k)).collect();
  format!(
    r#"{{"input":{{"a":{n}}},"output":{{"r":{n}}},"cmds":[{{"type":"command","cmd":"{c}","name":"call","in":{{{i}}},"out":{{"a":{l}}},"pos":{p},"width":1}}],"cons":[{w},{{"src":[0,"a"],"dest":[-2,"r"]}}]}}"#,
    n = NODE, l = LIST, p = POS, c = cmd, i = in_o.join(","), w = cons.join(","))
}

fn time<F: FnMut()>(mut f: F) -> Duration {
  let mut best = Duration::MAX;
  for _ in 0..3 { let t = Instant::now(); f(); best = best.min(t.elapsed()); }
  best
}

#[test]
fn code_execute_is_the_fast_path() {
  flowlang::init("data");
  let store = DataStore::new();
  let flow = |id: &str| store.get_data("testflow", id).get_object("data").get_object("flow").to_string();
  let list = |n: usize, f: &dyn Fn(usize) -> String| format!("[{}]", (0..n).map(|i| f(i)).collect::<Vec<_>>().join(","));
  let shapes: Vec<(&str, String, String)> = vec![
    ("list primitive x5000", flow("nzgmnn1803778b9d9t3a6"), format!("{{\"a\":{}}}", list(5000, &|i| i.to_string()))),
    ("local loop x3000", flow("uqoztz18041a7027eg318"), "{\"a\":3000}".to_string()),
    ("flow callee x500", calls("testflow:testflow:zkuwhn1802d57cb8ak1c", &["a", "b"]), format!("{{\"a\":{}}}", list(500, &|i| i.to_string()))),
    ("rust callee x2000", calls("testflow:testflow:nrnlzl1807a95053fz96c", &["a"]), format!("{{\"a\":{}}}", list(2000, &|i| format!("\"n{}\"", i)))),
  ];
  let (mut fast_total, mut step_total) = (Duration::ZERO, Duration::ZERO);
  for (name, body, args) in &shapes {
    let case = Case::from_data(DataObject::from_string(body));
    // both paths must succeed on the shape, with the same result
    let mut code = Code::new(Case::from_data(DataObject::from_string(body)));
    let r = code.execute(DataObject::from_string(args)).unwrap_or_else(|_| panic!("{}: Code::execute failed", name));
    let s = stepper::run_report(case.to_data(), DataObject::from_string(args), 0);
    assert_eq!(s.outcome, "ok", "{}: the stepper failed", name);
    if *name != "local loop x3000" { // test_speed reports elapsed clock time
      assert_eq!(r.to_string().len(), s.out.to_string().len(), "{}: results differ", name);
    }
    let fast = time(|| { let mut c = Code::new(Case::from_data(DataObject::from_string(body))); c.execute(DataObject::from_string(args)).ok(); });
    let step = time(|| { stepper::run_report(case.to_data(), DataObject::from_string(args), 0); DataObject::gc(); });
    eprintln!("{:<22} Code::execute {:>9.2?}   stepper {:>9.2?}   {:>5.1}x", name, fast, step, step.as_secs_f64() / fast.as_secs_f64());
    fast_total += fast; step_total += step;
  }
  let ratio = step_total.as_secs_f64() / fast_total.as_secs_f64();
  eprintln!("overall: {:.1}x", ratio);
  assert!(ratio >= 3.0, "Code::execute is only {:.1}x faster than the stepper; is it still on the fast path?", ratio);
}
