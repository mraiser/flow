use serde_json::*;
use std::path::Path;

mod code;
use code::Code as Code;

mod datastore;
use datastore::DataStore;

mod primitives;

fn main() {
  let path = Path::new("data");
  let store = DataStore::new(path.to_path_buf());
	let data = store.get_data("test", "qkjown179091cc94fz1a");
	let codeval = data["data"]["flow"].to_owned();
  let code = Code::new(codeval);
  
  let argstr = r#"
  {
    "a": 299,
    "b": 121
  }
  "#;
  let args: Value = serde_json::from_str(argstr).unwrap();
  let res = code.execute(args);

	println!("Hello, my dudes! {:?}", res);
}
