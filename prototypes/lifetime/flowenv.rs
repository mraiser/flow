use std::collections::HashMap;

use crate::heap::*;
use crate::data::*;
use crate::datastore::*;

#[derive(Debug)]
pub struct FlowEnv {
  pub obj_heap: Heap<HashMap<String,Data>>,
  pub store: DataStore,
}

impl FlowEnv {
  pub fn new(store:DataStore) -> FlowEnv {
    FlowEnv{
      obj_heap: Heap::new(),
      store: store,
    }
  }
}

