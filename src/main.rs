use serde_json::*;
use std::path::Path;
use std::env;

mod code;
use code::Code as Code;

mod datastore;
use datastore::DataStore;

mod primitives;

mod bytesref;
use bytesref::*;

mod bytesutil;

mod heap;

mod dataproperty;

mod dataobject;
use dataobject::*;

mod dataarray;

fn main() {
  env::set_var("RUST_BACKTRACE", "1");
  {
    let path = Path::new("data");
    let store = DataStore::new(path.to_path_buf());
    let data = store.get_data("test", "qkjown179091cc94fz1a");
    let codeval = data.get_object("data").get_object("flow");
    let code = Code::new(codeval);
    
    let argstr = r#"
    {
      "a": 299,
      "b": 121
    }
    "#;
    
    let args: Value = serde_json::from_str(argstr).unwrap();
    let res = code.execute(DataObject::from_json(args));
    println!("Hello, my dudes! {:?}", res);
    
//    BytesRef::print_heap();
  }
  
  BytesRef::print_heap();
}
