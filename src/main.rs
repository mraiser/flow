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
    println!("Hello, my dudes! {:?}", res);

    let args = DataObject::from_json(serde_json::from_str(r#"
    {
      "a": 210
    }
    "#).unwrap());
    let cmd = Command::new("testflow", "vnpvxv1802d67b7d1j1f", store.clone());
    let res = cmd.execute(args);
    println!("Hello, my dudes! {:?}", res);
    
//    BytesRef::print_heap();
  }
  
  BytesRef::print_heap();
}
