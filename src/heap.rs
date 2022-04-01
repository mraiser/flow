use std::collections::HashMap;
use crate::bytesref::*;

#[derive(Debug)]
pub struct Heap {
  pub data: HashMap<usize, Vec<u8>>,
  count: HashMap<usize, usize>,
  ref_index: usize,
  prop_lookup: HashMap<String, usize>,
  props: Vec<String>,
}

impl Heap {
  pub fn new() -> Heap {
    Heap {
      data: HashMap::<usize, Vec<u8>>::new(),
      count: HashMap::<usize, usize>::new(),
      ref_index: 0,
      prop_lookup: HashMap::<String, usize>::new(),
      props: Vec::<String>::new(),
    }
  }

  pub fn push(&mut self, bytes: Vec<u8>) -> BytesRef {
    let index = self.ref_index;
    self.ref_index += 1;
    let len = bytes.len() as usize;
    self.data.insert(index, bytes);
    self.count.insert(index, 1);
    BytesRef::new(index, 0, len)
  }
  
  pub fn child(&mut self, index: usize, off: usize, len: usize) -> BytesRef {
    let c = self.count[&index] + 1;
    self.count.insert(index, c);
    //println!("c New count for {}: {}", index, c);
    BytesRef::new(index, off, len)
  }
  
  pub fn incr(&mut self, index:usize) {
    let c = self.count[&index];
    self.count.insert(index, c+1);
    //println!("+ New count for {}: {}", index, c+1);
  }
  
  pub fn decr(&mut self, index: usize) {
    let c = self.count[&index];
    if c == 1 {
      self.data.remove(&index);
      self.count.remove(&index);
      //println!("Removing {}", index);
    }
    else {
      self.count.insert(index, c-1);
      //println!("- New count for {}: {}", index, c-1);
    }
  }
  
  pub fn swap(&mut self, index:usize, bytes:Vec<u8>) {
    self.data.insert(index, bytes);
  }
  
  pub fn lookup_prop_string(&self, i: usize) -> String {
    self.props[i].to_owned()
  }
  
  pub fn lookup_prop(&mut self, name: &str) -> usize {
    if let Some(i) = self.prop_lookup.get(name) {
      return i.to_owned();
    }
    else {
      let i = self.props.len() as usize;
      self.props.push(name.to_string());
      self.prop_lookup.insert(name.to_string(), i);
      return i;
    }
  }
}

