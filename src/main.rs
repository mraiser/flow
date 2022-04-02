use serde_json::*;
use std::path::Path;
use std::rc::Rc;

mod code;
use code::Code as Code;

mod datastore;
use datastore::DataStore;

mod primitives;

mod bytesref;
use bytesref::*;

mod bytesutil;
use bytesutil::*;

mod heap;
use heap::*;

mod dataproperty;
use dataproperty::*;

mod dataobject;
use dataobject::*;

mod dataarray;
use dataarray::*;

fn main() {
  if false {
    let argstr = r#"
    {
      "a": 299,
      "b": 121,
      "c": {
        "x": 10,
        "y": {
          "z": 7
        }
      }
    }
    "#;
    let args: Value = serde_json::from_str(argstr).unwrap();
    let o = DataObject::from_json(args);
    println!("OK: {:?} {:?}", o.get_i64("a"), o.get_i64("b"));

    BytesRef::print_heap();
  }
  else if true {
    let path = Path::new("data");
    let store = DataStore::new(path.to_path_buf());
    let data = store.get_data("test", "qkjown179091cc94fz1a");
    let codeval = data["data"]["flow"].to_string();
    let code = Code::new(&codeval).unwrap();
    
    let argstr = r#"
    {
      "a": 299,
      "b": 121
    }
    "#;
    let args: Value = serde_json::from_str(argstr).unwrap();
    
    let res = code.execute(args);
    println!("Hello, my dudes! {:?}", res);
    
    let o = DataObject::from_json(data);
    BytesRef::print_heap();
    println!("Hello, my dudes! {:?}", o);
  }
  else {
    let mut bytes = Vec::<u8>::new();
    bytes.push(0);
    bytes.push(0);
    bytes.push(0);
    bytes.push(0);
    bytes.push(12);
    bytes.push(22);
    bytes.push(32);
    bytes.push(42);
    
    {
      let mut ba1 = BytesRef::push(bytes);
      println!("YO {:?}", ba1);
      let mut ba2 = ba1.child(2,4);
      println!("YO {:?}", ba2);
      let mut ba3 = ba1.child(3,4);
      println!("YO {:?}", ba3);
    }
    
    BytesRef::lookup_prop("position");
    BytesRef::lookup_prop("rotation");
    
    
   let mut ba1 = BytesRef::push(Vec::<u8>::new());
   //println!("YO {:?}", &heap);
  }
  
  BytesRef::print_heap();
}
