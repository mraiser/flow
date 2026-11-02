//! A Python command whose impl record lacks "ctl" and "cmd" - as records
//! written by newbound's dev.code are - is found, runs and checks. (Before,
//! PyCmd::get_path read impl.ctl and panicked: "Key 'ctl' not found".) Its
//! own binary: a temporary store.

use flowlang::command::Command;
use flowlang::datastore::DataStore;
use flowlang::pycmd::{check, check_quick, PyCmd};
use ndata::dataobject::DataObject;

fn record(json: &str) -> DataObject { DataObject::from_string(json) }

#[test]
fn records_without_ctl_and_cmd_on_the_impl() {
  let root = std::env::temp_dir().join(format!("flowlang_py_records_{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&root);
  std::fs::create_dir_all(root.join("data").join("pl")).unwrap();
  DataStore::init(root.join("data").to_str().unwrap());
  let store = DataStore::new();
  store.set_data("pl", "controls", record(r#"{"data":{"list":[{"name":"other","id":"ctl0"},{"name":"tools","id":"ctl1"}]}}"#));
  store.set_data("pl", "ctl0", record(r#"{"data":{"cmd":[]}}"#));
  store.set_data("pl", "ctl1", record(r#"{"data":{"cmd":[{"name":"hello","id":"cmd1"}]}}"#));
  store.set_data("pl", "cmd1", record(r#"{"data":{"type":"python","python":"impl1","name":"hello"}}"#));
  store.set_data("pl", "impl1", record(r#"{"data":{"params":[{"name":"who","type":"String"}],"returntype":"String"}}"#));

  // No "root" in meta.json: the library's crate is <data>/../cmd.
  let path = PyCmd::get_path("pl", "cmd1");
  assert!(path.ends_with("cmd/src/pl/tools/hello.py"), "{}", path);
  assert!(!Command::exists("pl", "cmd1"));
  std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap()).unwrap();
  std::fs::write(&path, "\n\ndef execute(args):\n    return hello(args['who'])\n\ndef hello(who: \"str\") -> \"str\":\n    return 'hi ' + who\n").unwrap();
  assert!(Command::exists("pl", "cmd1"));

  let mut args = DataObject::new();
  args.put_string("who", "there");
  let r = Command::new("pl", "cmd1").execute(args).unwrap();
  assert_eq!(r.to_string(), r#"{"a":"hi there"}"#);

  for r in [check("pl", "cmd1"), check_quick("pl", "cmd1")] {
    assert_eq!(r.get_string("status"), "ok", "{}", r.to_string());
    assert_eq!(r.get_string("command"), "pl:tools:hello");
    assert!(r.get_boolean("clean"), "{}", r.to_string());
  }
  assert_eq!(check_quick("pl", "cmd1").get_string("mypy"), "skipped");
  let _ = std::fs::remove_dir_all(&root);
}
