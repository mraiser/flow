use std::collections::HashMap;

use crate::heap::*;
use crate::data::*;
use crate::datastore::*;

pub static mut FLOWENV:Vec<FlowEnv> = Vec::new();

#[derive(Debug)]
pub struct FlowEnv {
  pub obj_heap: Heap<HashMap<String,Data>>,
  pub store: DataStore,
}

impl FlowEnv {
  pub fn init(store:DataStore) {
    let f = FlowEnv{
      obj_heap: Heap::new(),
      store: store,
    };
    unsafe {
      FLOWENV.push(f);
    }
  }
}

