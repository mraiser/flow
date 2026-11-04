//! `Command::cast_params` / `try_cast_params` cast arguments to the declared parameter types
//! (app.app.exec, dev.code.invoke_command, newbound's websocket path and `flowlang.call`
//! all cast before they execute). A value comes out as the generated command wrapper reads it
//! (`builder::util::lookup_rust_api_data_type`):
//! - Integer, Float, Boolean, JSONObject and JSONArray are converted (parsed from a string);
//! - Any (read with `get_property`) and InputStream (read with `get_bytes`) pass through
//!   unchanged;
//! - String and every other type become a string.
//!
//! Before, Any and InputStream became strings too: a list passed to a flow's `Any` parameter
//! (the type flow editors give an untyped bar) arrived as the text "[1,2,3]".

use flowlang::case::Case;
use flowlang::command::{Command, Source};
use ndata::data::Data;
use ndata::databytes::DataBytes;
use ndata::dataobject::DataObject;

/// A flow command `t` with the given declared parameters whose body passes `xs` through a
/// list-mode `+ 1`, so it only works when `xs` arrives as a list.
fn command(params: &[(&str, &str)]) -> Command {
  let body = r#"{"input":{"xs":{"mode":"regular","type":"Any","x":0}},"output":{"r":{"mode":"regular","type":"Any","x":0}},
    "cmds":[
      {"type":"primitive","name":"+","in":{"a":{"mode":"list","type":"Any","x":0},"b":{"mode":"regular","type":"Any","x":0}},"out":{"a":{"mode":"list","type":"Any","x":0}},"pos":{"x":0,"y":0,"z":0},"width":1},
      {"type":"constant","ctype":"int","name":"1","in":{},"out":{"a":{"mode":"regular","type":"Any","x":0}},"pos":{"x":0,"y":0,"z":0},"width":1}],
    "cons":[{"src":[-1,"xs"],"dest":[0,"a"]},{"src":[1,"a"],"dest":[0,"b"]},{"src":[0,"a"],"dest":[-2,"r"]}]}"#;
  Command {
    name: "t".to_string(), lib: "castlib".to_string(), id: "t".to_string(), lang: "flow".to_string(),
    src: Source::Flow(Case::from_data(DataObject::from_string(body))),
    return_type: "Any".to_string(),
    params: params.iter().map(|(n, t)| (n.to_string(), t.to_string())).collect(),
    readers: Vec::new(), writers: Vec::new(),
  }
}

fn cast(params: &[(&str, &str)], args: DataObject) -> DataObject {
  command(params).try_cast_params(args.clone()).expect("the arguments fit");
  args
}

#[test]
fn cast_params_keeps_any_and_bytes() {
  let dir = std::env::temp_dir().join("flowlang_cast_params_test");
  let _ = std::fs::create_dir_all(&dir);
  flowlang::datastore::DataStore::init(dir.to_str().unwrap());

  // Any: every kind of value passes through as itself.
  let args = DataObject::from_string(r#"{"l":[1,2],"o":{"k":1},"i":5,"f":1.5,"b":true,"s":"x","n":null}"#);
  let any: Vec<(&str, &str)> = ["l", "o", "i", "f", "b", "s", "n"].iter().map(|k| (*k, "Any")).collect();
  let a = cast(&any, args);
  assert!(a.get_property("l").is_array(), "a list stays a list: {}", a.to_string());
  assert!(a.get_property("o").is_object(), "an object stays an object: {}", a.to_string());
  assert_eq!(a.get_property("i"), Data::DInt(5));
  assert_eq!(a.get_property("f"), Data::DFloat(1.5));
  assert_eq!(a.get_property("b"), Data::DBoolean(true));
  assert_eq!(a.get_property("s"), Data::DString("x".to_string()));
  assert!(a.get_property("n").is_null());
  // a missing Any parameter is null (unchanged)
  let a = cast(&[("gone", "Any")], DataObject::new());
  assert!(a.has("gone") && a.get_property("gone").is_null());

  // InputStream: bytes stay bytes, which is what the wrapper's get_bytes reads.
  let mut args = DataObject::new();
  args.put_bytes("st", DataBytes::from_bytes(&vec![1u8, 2, 3]));
  let a = cast(&[("st", "InputStream")], args);
  assert_eq!(a.get_bytes("st").get_data(), vec![1u8, 2, 3]);

  // The other types are unchanged: converted, or made strings.
  let a = cast(&[("i", "Integer"), ("f", "Float"), ("b", "Boolean"), ("s", "String"), ("u", "Unknown"), ("o", "JSONObject"), ("l", "JSONArray")],
    DataObject::from_string(r#"{"i":"7","f":"2.5","b":"true","s":5,"u":[1],"o":"{\"k\":1}","l":"[1,2]"}"#));
  assert_eq!(a.get_property("i"), Data::DInt(7));
  assert_eq!(a.get_property("f"), Data::DFloat(2.5));
  assert_eq!(a.get_property("b"), Data::DBoolean(true));
  assert_eq!(a.get_property("s"), Data::DString("5".to_string()));
  assert_eq!(a.get_property("u"), Data::DString("[1]".to_string()));
  assert_eq!(a.get_object("o").get_int("k"), 1);
  assert_eq!(a.get_array("l").len(), 2);

  // End to end: a flow whose Any parameter is a list runs (it failed with "not an array").
  let c = command(&[("xs", "Any")]);
  let args = DataObject::from_string(r#"{"xs":[1,2,3]}"#);
  c.try_cast_params(args.clone()).expect("the arguments fit");
  let r = c.execute(args).unwrap_or_else(|e| panic!("the flow fails: {}", e));
  assert_eq!(r.to_string(), r#"{"r":[2,3,4]}"#);
}
