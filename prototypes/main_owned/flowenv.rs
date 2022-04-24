use std::collections::HashMap;
use std::sync::RwLock;
use state::Storage;

use crate::heap::*;
use crate::data::*;
use crate::dataobject::*;
use crate::dataarray::*;
use crate::datastore::*;

pub static ODROP:Storage<RwLock<Vec<usize>>> = Storage::new();
pub static ADROP:Storage<RwLock<Vec<usize>>> = Storage::new();

#[derive(Debug)]
pub struct FlowEnv {
  pub objects: Heap<HashMap<String,Data>>,
  pub arrays: Heap<Vec<Data>>,
  pub store: DataStore,
}

impl FlowEnv {
  pub fn init() {
    ODROP.set(RwLock::new(Vec::new()));
    ADROP.set(RwLock::new(Vec::new()));
  }
  
  pub fn gc(&mut self) {
    {
      let odrop = &mut ODROP.get().write().unwrap();
      let mut i = odrop.len();
      while i>0 {
        i = i - 1;
        let x = odrop.remove(0);
        DataObject::delete(self, x);
      }
    }
    let adrop = &mut ADROP.get().write().unwrap();
    let mut i = adrop.len();
    while i>0 {
      i = i - 1;
      let x = adrop.remove(0);
      DataArray::delete(self, x);
    }
  }
}

