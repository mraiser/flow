//! Command::exists for a Python command means its generated source exists.
//! (Was always true - a FIXME.) The appserver's timer loop waits for a
//! command to exist before running it, as it does for Rust. Its own binary:
//! it briefly moves testflow's generated test_python.py aside.

use flowlang::command::Command;

#[test]
fn command_exists_means_the_generated_source_exists() {
  flowlang::init("data");
  let id = "ovgzjp1822699e1d1w19"; // testflow:testflow:test_python
  assert!(Command::exists("testflow", id));
  let path = flowlang::pycmd::PyCmd::get_path("testflow", id);
  let aside = format!("{}.aside", path);
  std::fs::rename(&path, &aside).unwrap();
  let missing = Command::exists("testflow", id);
  std::fs::rename(&aside, &path).unwrap();
  assert!(!missing);
  assert!(Command::exists("testflow", id));
}
