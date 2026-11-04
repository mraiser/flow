//! Flow step debugger P6: Code::execute is a continue-to-end over the stepper.
//! I-4 (next with no next case) and a stuck case are now Err(CodeException::Fail(..)),
//! not a panic / hang.

use flowlang::case::Case;
use flowlang::code::{Code, CodeException};
use flowlang::datastore::DataStore;
use ndata::dataobject::DataObject;
use std::sync::Once;

static INIT: Once = Once::new();
fn init() {
  INIT.call_once(|| {
    let dir = std::env::temp_dir().join("flowlang_stepper_p6_test");
    let _ = std::fs::create_dir_all(&dir);
    DataStore::init(dir.to_str().unwrap());
  });
}

fn run(body: &str, args: &str) -> Result<DataObject, CodeException> {
  let mut code = Code::new(Case::new(body));
  code.execute(DataObject::from_string(args))
}

const I4: &str = "{\"cmds\":[{\"condition\":{\"rule\":\"next\",\"value\":true},\"in\":{},\"name\":\"go_next\",\"out\":{},\"pos\":{\"x\":0,\"y\":0,\"z\":0},\"type\":\"local\",\"width\":1.5}],\"cons\":[],\"input\":{},\"nextcase\":{\"cmds\":[{\"condition\":{\"rule\":\"next\",\"value\":true},\"in\":{},\"name\":\"next_again\",\"out\":{},\"pos\":{\"x\":0,\"y\":0,\"z\":0},\"type\":\"local\",\"width\":1.5}],\"cons\":[],\"input\":{},\"output\":{}},\"output\":{}}";

#[test]
fn i4_next_in_last_case_is_an_error_not_a_panic() {
  init();
  let r = run(I4, "{}");
  assert!(r.is_err(), "I-4 must return Err");
  match r.err() {
    Some(CodeException::Fail(e)) => assert_eq!(e.code, "I-4", "{}", e),
    other => panic!("expected an I-4 Fail, got {:?}", other),
  }
}

// A wire cycle (I-2): op 0 waits for op 1 and op 1 for op 0. (This fixture used to be an unwired
// input sharing a wired name; since flowlang 0.3.47 such an input is null, not awaited.)
const STUCK: &str = "{\"cmds\":[{\"in\":{\"x\":{\"mode\":\"regular\",\"type\":\"Any\",\"x\":0}},\"name\":\"a\",\"out\":{\"y\":{\"mode\":\"regular\",\"type\":\"Any\",\"x\":0}},\"pos\":{\"x\":0,\"y\":0,\"z\":0},\"width\":1.5,\"type\":\"local\"},{\"in\":{\"x\":{\"mode\":\"regular\",\"type\":\"Any\",\"x\":0}},\"name\":\"b\",\"out\":{\"y\":{\"mode\":\"regular\",\"type\":\"Any\",\"x\":0}},\"pos\":{\"x\":0,\"y\":0,\"z\":0},\"width\":1.5,\"type\":\"local\"}],\"cons\":[{\"dest\":[1,\"x\"],\"src\":[0,\"y\"]},{\"dest\":[0,\"x\"],\"src\":[1,\"y\"]},{\"dest\":[-2,\"r\"],\"src\":[1,\"y\"]}],\"input\":{},\"output\":{\"r\":{\"mode\":\"regular\",\"type\":\"Any\",\"x\":0}}}";

#[test]
fn stuck_flow_is_an_error_not_a_hang() {
  init();
  let r = run(STUCK, "{\"a\":\"hi\"}");
  assert!(r.is_err(), "a stuck flow must return Err");
}

const ADD: &str = "{\"cmds\":[{\"in\":{\"a\":{\"mode\":\"in\",\"type\":\"Any\",\"x\":0},\"b\":{\"mode\":\"in\",\"type\":\"Any\",\"x\":0}},\"name\":\"+\",\"out\":{\"a\":{\"mode\":\"out\",\"type\":\"Any\",\"x\":0}},\"pos\":{\"x\":0,\"y\":0,\"z\":0},\"type\":\"primitive\",\"width\":1.5}],\"cons\":[{\"dest\":[0,\"a\"],\"src\":[-1,\"x\"]},{\"dest\":[0,\"b\"],\"src\":[-1,\"y\"]},{\"dest\":[-2,\"r\"],\"src\":[0,\"a\"]}],\"input\":{\"x\":{\"mode\":\"in\",\"type\":\"Any\",\"x\":0},\"y\":{\"mode\":\"in\",\"type\":\"Any\",\"x\":0}},\"output\":{\"r\":{\"mode\":\"out\",\"type\":\"Any\",\"x\":0}}}";

#[test]
fn normal_flow_parity() {
  init();
  let r = run(ADD, "{\"x\":2,\"y\":3}").expect("normal flow must succeed");
  assert_eq!(r.get_int("r"), 5);
}

const TERM: &str = "{\"cmds\":[{\"condition\":{\"rule\":\"terminate\",\"value\":true},\"ctype\":\"string\",\"in\":{\"a\":{\"mode\":\"in\",\"type\":\"Any\",\"x\":0}},\"name\":\"stop\",\"out\":{},\"pos\":{\"x\":-1,\"y\":0,\"z\":0},\"type\":\"match\",\"width\":1.5},{\"ctype\":\"string\",\"in\":{},\"name\":\"before\",\"out\":{\"c\":{\"mode\":\"out\",\"type\":\"String\",\"x\":0}},\"pos\":{\"x\":1,\"y\":0,\"z\":0},\"type\":\"constant\",\"width\":1.5}],\"cons\":[{\"dest\":[0,\"a\"],\"src\":[-1,\"v\"]},{\"dest\":[-2,\"r\"],\"src\":[1,\"c\"]}],\"input\":{\"v\":{\"mode\":\"in\",\"type\":\"String\",\"x\":0}},\"output\":{\"r\":{\"mode\":\"out\",\"type\":\"String\",\"x\":0}}}";

#[test]
fn terminate_ends_the_flow_normally() {
  init();
  let r = run(TERM, "{\"v\":\"stop\"}");
  assert!(r.is_ok(), "terminate at the root must end the flow");
}
