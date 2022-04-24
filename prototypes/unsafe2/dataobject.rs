use serde_json::*;
use std::collections::HashMap;
use once_cell::sync::Lazy; // 1.3.1
use std::sync::Mutex;
use std::fmt;

use crate::heap::*;
use crate::data::*;
use crate::flowenv::*;

pub struct DataObject {
  pub data_ref: usize,
}

impl DataObject {
  pub fn new() -> DataObject {
    unsafe {
      let data_ref = FLOWENV[0].obj_heap.push(HashMap::<String,Data>::new());
      return DataObject {
        data_ref: data_ref,
      };
    }
  }
  
  pub fn get(data_ref: usize) -> DataObject {
    let mut o = DataObject{
      data_ref: data_ref,
    };
    o.incr();
    o
  }
  
  pub fn incr(&mut self) {
    unsafe {
      FLOWENV[0].obj_heap.incr(self.data_ref);
    }
  }
  
  pub fn decr(&mut self) {
    unsafe {
      FLOWENV[0].obj_heap.decr(self.data_ref);
    }
  }

  pub fn print_heap() {
    unsafe {
      println!("{:?}", FLOWENV);
    }
  }
}

impl Drop for DataObject {
  fn drop(&mut self) {
    println!("DROP {}", self.data_ref);
    self.decr();
  }
}
