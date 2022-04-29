use std::path::Path;
use std::env;

mod code;
mod case;
mod command;
mod datastore;
mod primitives;
mod data;
mod heap;
mod dataobject;
mod dataarray;
mod flowenv;
mod usizemap;

use command::Command as Command;
use datastore::DataStore;
use dataobject::*;
use dataarray::*;
use flowenv::*;

fn main() {
  let path = Path::new("data");
  let store = DataStore::new(path.to_path_buf());
  
  FlowEnv::init(store);
  
  env::set_var("RUST_BACKTRACE", "1");
  {
    let args = DataObject::from_json(serde_json::from_str(r#"
    {
      "a": 299,
      "b": 121
    }
    "#).unwrap());
    let cmd = Command::new("testflow", "zkuwhn1802d57cb8ak1c");
    let res = cmd.execute(args);
    println!("test_add: {:?}", res.unwrap().to_json());

    let args = DataObject::from_json(serde_json::from_str(r#"
    {
      "a": 210
    }
    "#).unwrap());
    let cmd = Command::new("testflow", "vnpvxv1802d67b7d1j1f");
    let res = cmd.execute(args);
    println!("test_command: {:?}", res.unwrap().to_json());
    
    let args = DataObject::from_json(serde_json::from_str(r#"
    {
      "a": true
    }
    "#).unwrap());
    let cmd = Command::new("testflow", "ooizjt1803765b08ak212");
    let res = cmd.execute(args);
    println!("test_conditionals: {:?}", res.unwrap().to_json());
    
    let args = DataObject::from_json(serde_json::from_str(r#"
    {
      "a": [1,2,3]
    }
    "#).unwrap());
    let cmd = Command::new("testflow", "izzpiy1803778a841p3a5");
    let res = cmd.execute(args);
    println!("test_lists: {:?}", res.unwrap().to_json());
    
    let args = DataObject::from_json(serde_json::from_str(r#"
    {
      "a": 0
    }
    "#).unwrap());
    let cmd = Command::new("testflow", "izmuzm18037d796f1i467");
    let res = cmd.execute(args);
    println!("test_loop: {:?}", res.unwrap().to_json());
    
    let args = DataObject::from_json(serde_json::from_str(r#"
    {
      "a": 100000
    }
    "#).unwrap());
    let cmd = Command::new("testflow", "jqlvrz18041a69d0bw311");
    let res = cmd.execute(args);
    println!("test_speed: {}", res.unwrap().to_json());

    DataObject::print_heap();
    DataArray::print_heap();
  }
//  DataObject::print_heap();
//  DataArray::print_heap();

  {    
    let env = &mut FLOWENV.get().write().unwrap();
    env.gc();
  }

  DataObject::print_heap();
  DataArray::print_heap();
}
