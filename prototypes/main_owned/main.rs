use std::path::Path;
use std::env;

mod code;
mod command;
mod datastore;
mod primitives;
mod data;
mod heap;
mod dataobject;
mod dataarray;
mod flowenv;

use command::Command as Command;
use datastore::DataStore;
use dataobject::*;
use dataarray::*;
use flowenv::*;
use heap::*;

fn main() {
  let path = Path::new("data");
  let store = DataStore::new(path.to_path_buf());
  
  FlowEnv::init();
  
  let env = &mut FlowEnv{
    objects: Heap::new(),
    arrays: Heap::new(),
    store: store,
  };


  env::set_var("RUST_BACKTRACE", "1");
  {
    let args = DataObject::from_json(serde_json::from_str(r#"
    {
      "a": 299,
      "b": 121
    }
    "#).unwrap(), env);
    let cmd = Command::new("testflow", "zkuwhn1802d57cb8ak1c", env);
    let res = cmd.execute(args, env);
    println!("test_add: {:?}", res);

    let args = DataObject::from_json(serde_json::from_str(r#"
    {
      "a": 210
    }
    "#).unwrap(), env);
    let cmd = Command::new("testflow", "vnpvxv1802d67b7d1j1f", env);
    let res = cmd.execute(args, env);
    println!("test_command: {:?}", res);
    
    let args = DataObject::from_json(serde_json::from_str(r#"
    {
      "a": true
    }
    "#).unwrap(), env);
    let cmd = Command::new("testflow", "ooizjt1803765b08ak212", env);
    let res = cmd.execute(args, env);
    println!("test_conditionals: {:?}", res);
    
    let args = DataObject::from_json(serde_json::from_str(r#"
    {
      "a": [1,2,3]
    }
    "#).unwrap(), env);
    let cmd = Command::new("testflow", "izzpiy1803778a841p3a5", env);
    let res = cmd.execute(args, env);
    println!("test_lists: {:?}", res);
    
    let args = DataObject::from_json(serde_json::from_str(r#"
    {
      "a": 0
    }
    "#).unwrap(), env);
    let cmd = Command::new("testflow", "izmuzm18037d796f1i467", env);
    let res = cmd.execute(args, env);
    println!("test_loop: {:?}", res);
    
    let args = DataObject::from_json(serde_json::from_str(r#"
    {
      "a": 1000
    }
    "#).unwrap(), env);
    let cmd = Command::new("testflow", "jqlvrz18041a69d0bw311", env);
    let res = cmd.execute(args, env);
    println!("test_speed: {}", res.unwrap().get_i64("a", env));

//    DataObject::print_heap(env);
//    DataArray::print_heap(env);
  }
//  DataObject::print_heap(env);
//  DataArray::print_heap(env);
  env.gc();
  DataObject::print_heap(env);
  DataArray::print_heap(env);
}
