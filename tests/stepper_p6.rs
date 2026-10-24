//! Flow step debugger P6: Code::execute is a continue-to-end over the stepper.
//! I-4 (next with no next case) and a stuck case are now Err(CodeException::Fail),
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
  assert_eq!(r.err(), Some(CodeException::Fail));
}

const STUCK: &str = "{\"cmds\":[{\"in\":{\"x\":{\"mode\":\"in\",\"type\":\"Any\",\"x\":0}},\"name\":\"takes_x\",\"out\":{\"y\":{\"mode\":\"out\",\"type\":\"Any\",\"x\":0}},\"pos\":{\"x\":-1,\"y\":0,\"z\":0},\"type\":\"local\",\"width\":1.5},{\"in\":{\"x\":{\"mode\":\"in\",\"type\":\"Any\",\"x\":0}},\"name\":\"orphan\",\"out\":{\"z\":{\"mode\":\"out\",\"type\":\"Any\",\"x\":0}},\"pos\":{\"x\":1,\"y\":0,\"z\":0},\"type\":\"local\",\"width\":1.5}],\"cons\":[{\"dest\":[0,\"x\"],\"src\":[-1,\"a\"]},{\"dest\":[-2,\"q\"],\"src\":[0,\"y\"]},{\"dest\":[-2,\"r\"],\"src\":[1,\"z\"]}],\"input\":{\"a\":{\"mode\":\"in\",\"type\":\"String\",\"x\":0}},\"output\":{\"q\":{\"mode\":\"out\",\"type\":\"Any\",\"x\":-0.3},\"r\":{\"mode\":\"out\",\"type\":\"Any\",\"x\":0.3}}}";

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
