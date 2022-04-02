use std::collections::HashMap;
use once_cell::sync::Lazy; // 1.3.1
use std::sync::Mutex;

use crate::heap::*;
use crate::bytesutil::*;
use crate::dataproperty::*;

static HEAP: Lazy<Mutex<Heap>> = Lazy::new(|| Mutex::new(Heap::new()));

#[derive(Debug)]
pub struct BytesRef {
  pub byte_ref: usize,
  pub off: usize,
  pub len:usize,
}

impl BytesRef {
  pub fn new(byte_ref: usize, off:usize, len:usize) -> BytesRef {
    return BytesRef {
      byte_ref: byte_ref,
      off: off,
      len: len,
    };
  }
  
  pub fn push(bytes: Vec<u8>) -> BytesRef {
    HEAP.lock().unwrap().push(bytes)
  }
      
  pub fn get(index:usize, off: usize, len: usize) -> BytesRef {
    HEAP.lock().unwrap().child(index, off, len)
  }
  
  pub fn to_handle_bytes(&self) -> Vec<u8> {
    let mut bytes = Vec::<u8>::new();
    bytes.append(&mut i64_to_bytes(self.byte_ref as i64));
    bytes.append(&mut i64_to_bytes(self.off as i64));
    bytes.append(&mut i64_to_bytes(self.len as i64));
    bytes
  }

  pub fn to_handle(&self) -> BytesRef {
    self.incr();
    let mut bytes = self.to_handle_bytes();
    BytesRef::push(bytes)
  }

  pub fn from_handle(&mut self) -> BytesRef {
    let bytes = self.get_bytes();
    let byte_ref: usize = bytes_to_i64(&bytes, 0) as usize;
    let off: usize = bytes_to_i64(&bytes, 8) as usize;
    let len: usize = bytes_to_i64(&bytes, 16) as usize;
    BytesRef::get(byte_ref, off, len)
  }
  
  pub fn swap(&self, bytes:Vec<u8>) {
    HEAP.lock().unwrap().swap(self.byte_ref, bytes);
  }
        
  pub fn child(&mut self, off: usize, len: usize) -> BytesRef {
    HEAP.lock().unwrap().child(self.byte_ref, self.off + off, len)
  }
  
  pub fn duplicate(&mut self) -> BytesRef {
    HEAP.lock().unwrap().child(self.byte_ref, self.off, self.len)
  }
  
  pub fn lookup_prop(name: &str) -> usize {
    HEAP.lock().unwrap().lookup_prop(name)
  }

  pub fn lookup_prop_string(i: usize) -> String {
    HEAP.lock().unwrap().lookup_prop_string(i)
  }
  
  pub fn from_str(s:&str) -> BytesRef {
    let bytes: Vec<u8> = s.as_bytes().to_vec();
    HEAP.lock().unwrap().push(bytes)
  }
    
  pub fn from_bool(b:bool) -> BytesRef {
    let mut bytes: Vec<u8> = vec![0];
    if b { bytes[0] = 1; }
    HEAP.lock().unwrap().push(bytes)
  }

  pub fn from_i64(val:i64) -> BytesRef {
    let mut bytes: Vec<u8> = i64_to_bytes(val);
    HEAP.lock().unwrap().push(bytes)
  }
  
  pub fn from_f64(val:f64) -> BytesRef {
    let i1:i32 = val as i32;
    let i2:i32 = (f32::MAX as f64 * (val - (i1 as f64))) as i32;
    let mut bytes = i32_to_bytes(i1);
    bytes.append(&mut i32_to_bytes(i2));
    HEAP.lock().unwrap().push(bytes)
  }
  
  pub fn as_i32(&self) -> i32{
    let bytes = self.get_bytes();
    let mut i = 0;
    let mut val:i32 = 0;
    while i<4 {
      let shift = (3 - i) * 8;
      val += ((bytes[i + self.off] as i32) & 0xFF) << shift;
      i = i + 1;
    }
    val
  }
  
  pub fn as_i64(&self) -> i64{
    let bytes = self.get_bytes();
    bytes_to_i64(&bytes, self.off)
  }

  pub fn as_properties(&self) -> HashMap<usize, DataProperty>{
    bytes_to_properties(self.get_bytes(), self.off, self.len)
  }  
  
  pub fn count(&self) -> usize {
    HEAP.lock().unwrap().count(self.byte_ref)
  }
  
  pub fn incr(&self) {
    HEAP.lock().unwrap().incr(self.byte_ref);
  }
  
  pub fn decr(&self) {
    HEAP.lock().unwrap().decr(self.byte_ref);
  }
  
  pub fn print_heap() {
    println!("{:?}", HEAP);
  }
  
  fn get_bytes(&self) -> Vec<u8> {
    HEAP.lock().unwrap().data.get(&self.byte_ref).unwrap().to_owned()
  }
}

impl Drop for BytesRef {
  fn drop(&mut self) {
    HEAP.lock().unwrap().decr(self.byte_ref);
  }
}

