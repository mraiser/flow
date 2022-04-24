use std::path::Path;
use std::env;

mod data;
mod dataobject;
mod datastore;
mod flowenv;
mod heap;

use datastore::*;
use dataobject::*;
use flowenv::*;

fn main() {
  let path = Path::new("data");
  let store = DataStore::new(path.to_path_buf());
  FlowEnv::init(store);
  
  env::set_var("RUST_BACKTRACE", "1");
  {
    let do1 = DataObject::new();
    let do2 = DataObject::new();
    
    
    
    DataObject::print_heap();
//    DataArray::print_heap();
  }
  
  DataObject::print_heap();
//  DataArray::print_heap();
}
