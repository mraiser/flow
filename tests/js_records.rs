//! JavaScript commands found through store records, under both language
//! tags: `"js"` (flowlang's own) and `"javascript"` (what newbound's
//! dev.code writes). Before, a `"javascript"` record panicked with "Unknown
//! command type javascript". Its own binary: a temporary store.
#![cfg(feature = "javascript_runtime")]

use flowlang::command::Command;
use flowlang::datastore::DataStore;
use ndata::dataobject::DataObject;

fn record(json: &str) -> DataObject { DataObject::from_string(json) }

#[test]
fn records_under_both_language_tags() {
  let root = std::env::temp_dir().join(format!("flowlang_js_records_{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&root);
  std::fs::create_dir_all(root.join("data").join("jl")).unwrap();
  DataStore::init(root.join("data").to_str().unwrap());
  let store = DataStore::new();
  store.set_data("jl", "controls", record(r#"{"data":{"list":[{"name":"tools","id":"ctl1"}]}}"#));
  store.set_data("jl", "ctl1", record(r#"{"data":{"cmd":[{"name":"hello","id":"cmd1"},{"name":"add","id":"cmd2"},{"name":"broken","id":"cmd3"}]}}"#));

  // As newbound's dev.code.upsert_command writes it: type and keys "javascript",
  // no ctl or cmd on the impl record.
  store.set_data("jl", "cmd1", record(r#"{"data":{"type":"javascript","javascript":"impl1","name":"hello"}}"#));
  store.set_data("jl", "impl1", record(r#"{"data":{"type":"javascript","javascript":"return 'hi ' + who","params":[{"name":"who","type":"String"}],"returntype":"String"}}"#));
  // The original flowlang form: "js".
  store.set_data("jl", "cmd2", record(r#"{"data":{"type":"js","js":"impl2","name":"add"}}"#));
  store.set_data("jl", "impl2", record(r#"{"data":{"js":"return a + b","ctl":"tools","params":[{"name":"a","type":"Integer"},{"name":"b","type":"Integer"}],"returntype":"Integer"}}"#));
  // No body.
  store.set_data("jl", "cmd3", record(r#"{"data":{"type":"javascript","javascript":"impl3","name":"broken"}}"#));
  store.set_data("jl", "impl3", record(r#"{"data":{"params":[],"returntype":"String"}}"#));

  assert!(Command::exists("jl", "cmd1"));
  assert!(Command::exists("jl", "cmd2"));
  assert!(!Command::exists("jl", "cmd3"));

  let mut args = DataObject::new();
  args.put_string("who", "there");
  let r = Command::new("jl", "cmd1").execute(args).unwrap();
  assert_eq!(r.to_string(), r#"{"a":"hi there"}"#);

  let mut args = DataObject::new();
  args.put_int("a", 300);
  args.put_int("b", 120);
  let r = Command::lookup("jl", "tools", "add").execute(args).unwrap();
  assert_eq!(r.to_string(), r#"{"a":420}"#);

  // A declared param missing from the call is an error, in the envelope.
  let r = Command::new("jl", "cmd2").execute(DataObject::new()).unwrap();
  assert_eq!(r.get_object("a").get_string("status"), "err");
  assert_eq!(r.get_object("a").get_string("msg"), "missing required parameter: a");

  let r = Command::new("jl", "cmd3").execute(DataObject::new()).unwrap();
  assert_eq!(r.get_object("a").get_string("status"), "err");
  assert!(r.get_object("a").get_string("msg").contains("has no JavaScript body"), "{}", r.to_string());

  let _ = std::fs::remove_dir_all(&root);
}
