use std::collections::HashMap;
use std::sync::RwLock;
use state::Storage;

use crate::heap::*;
use crate::data::*;
use crate::datastore::*;

pub static FLOWENV:Storage<RwLock<FlowEnv>> = Storage::new();

#[derive(Debug)]
pub struct FlowEnv {
  pub objects: Heap<HashMap<String,Data>>,
  pub arrays: Heap<Vec<Data>>,
  pub store: DataStore,
}

impl FlowEnv {
  pub fn init(store:DataStore) {
    let f = FlowEnv{
      objects: Heap::new(),
      arrays: Heap::new(),
      store: store,
    };
    FLOWENV.set(RwLock::new(f));
  }
}

