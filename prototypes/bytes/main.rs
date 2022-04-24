use std::path::Path;
use std::env;

mod code;
mod command;
mod datastore;
mod primitives;
mod bytesref;
mod bytesutil;
mod heap;
mod dataproperty;
mod dataobject;
mod dataarray;

use command::Command as Command;
use datastore::DataStore;
use bytesref::*;
use dataobject::*;

fn main() {
  env::set_var("RUST_BACKTRACE", "1");
  {
    let path = Path::new("data");
    let store = DataStore::new(path.to_path_buf());
    
    let args = DataObject::from_json(serde_json::from_str(r#"
    {
      "a": 299,
      "b": 121
    }
    "#).unwrap());
    let cmd = Command::new("testflow", "zkuwhn1802d57cb8ak1c", store.clone());
    let res = cmd.execute(args);
    println!("test_add: {:?}", res);

    let args = DataObject::from_json(serde_json::from_str(r#"
    {
      "a": 210
    }
    "#).unwrap());
    let cmd = Command::new("testflow", "vnpvxv1802d67b7d1j1f", store.clone());
    let res = cmd.execute(args);
    println!("test_command: {:?}", res);
    
    let args = DataObject::from_json(serde_json::from_str(r#"
    {
      "a": true
    }
    "#).unwrap());
    let cmd = Command::new("testflow", "ooizjt1803765b08ak212", store.clone());
    let res = cmd.execute(args);
    println!("test_conditionals: {:?}", res);
    
    let args = DataObject::from_json(serde_json::from_str(r#"
    {
      "a": [1,2,3]
    }
    "#).unwrap());
    let cmd = Command::new("testflow", "izzpiy1803778a841p3a5", store.clone());
    let res = cmd.execute(args);
    println!("test_lists: {:?}", res);
    
    let args = DataObject::from_json(serde_json::from_str(r#"
    {
      "a": 0
    }
    "#).unwrap());
    let cmd = Command::new("testflow", "izmuzm18037d796f1i467", store.clone());
    let res = cmd.execute(args);
    println!("test_loop: {:?}", res);
    
    let args = DataObject::from_json(serde_json::from_str(r#"
    {
      "a": 100000
    }
    "#).unwrap());
    let cmd = Command::new("testflow", "jqlvrz18041a69d0bw311", store.clone());
    let res = cmd.execute(args);
    println!("test_speed: {:?}", res);
    
//    BytesRef::print_heap();
  }
  
  BytesRef::print_heap();
}
