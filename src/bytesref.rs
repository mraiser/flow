use once_cell::sync::Lazy; // 1.3.1
use std::sync::Mutex;

use crate::heap::*;

static HEAP: Lazy<Mutex<Heap>> = Lazy::new(|| Mutex::new(Heap::new()));

#[derive(Debug)]
pub struct BytesRef {
  byte_ref: usize,
  off: usize,
  len:usize,
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
//    let mut heap = HEAP.lock().unwrap();
//    let ba = heap.push(bytes);
//    println!("PUSH {:?}", &heap);
//    ba
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
    let mut bytes: Vec<u8> = Vec::<u8>::new();
    let mut i = 0;
    while i<8 {
      let shift = (7 - i) * 8;
      bytes.push(((val >> shift) & 0xFF) as u8);
      i = i + 1;
    }
    HEAP.lock().unwrap().push(bytes)
  }
  
  fn i32_to_bytes(val:i32) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::<u8>::new();
    let mut i = 0;
    while i<4 {
      let shift = (3 - i) * 8;
      bytes.push(((val >> shift) & 0xFF) as u8);
      i = i + 1;
    }
    bytes
  }

  pub fn from_f64(val:f64) -> BytesRef {
    let i1:i32 = val as i32;
    let i2:i32 = (f32::MAX as f64 * (val - (i1 as f64))) as i32;
    let mut bytes = BytesRef::i32_to_bytes(i1);
    bytes.append(&mut BytesRef::i32_to_bytes(i2));
    HEAP.lock().unwrap().push(bytes)
  }
      
  pub fn child(&mut self, off: usize, len: usize) -> BytesRef {
    HEAP.lock().unwrap().child(self.byte_ref, self.off + off, len)
  }
  
  pub fn duplicate(&mut self) -> BytesRef {
    HEAP.lock().unwrap().child(self.byte_ref, self.off, self.len)
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
    let mut i = 0;
    let mut val:i64 = 0;
    while i<8 {
      let shift = (7 - i) * 8;
      val += ((bytes[i + self.off] as i64) & 0xFF) << shift;
      i = i + 1;
    }
    val
  }
  
  fn get_bytes(&self) -> Vec<u8> {
    HEAP.lock().unwrap().data.get(&self.byte_ref).unwrap().to_owned()
  }
}

impl Drop for BytesRef {
  fn drop(&mut self) {
    HEAP.lock().unwrap().drop_ref(self.byte_ref);
//    let mut heap = HEAP.lock().unwrap();
//    heap.drop_ref(self.byte_ref);
//    println!("DROP {:?}", &heap);
  }
}

