use std::collections::HashMap;
use once_cell::sync::Lazy; // 1.3.1
use std::sync::Mutex;

use crate::heap::*;
use crate::dataproperty::*;

pub fn i64_to_bytes(val:i64) -> Vec<u8> {
  let mut bytes: Vec<u8> = Vec::<u8>::new();
  let mut i = 0;
  while i<8 {
    let shift = (7 - i) * 8;
    bytes.push(((val >> shift) & 0xFF) as u8);
    i = i + 1;
  }
  bytes
}

pub fn bytes_to_i64(bytes:&Vec<u8>, off:usize) -> i64{
  let mut i = 0;
  let mut val:i64 = 0;
  while i<8 {
    let shift = (7 - i) * 8;
    val += ((bytes[i + off] as i64) & 0xFF) << shift;
    i = i + 1;
  }
  val
}

pub fn i32_to_bytes(val:i32) -> Vec<u8> {
  let mut bytes: Vec<u8> = Vec::<u8>::new();
  let mut i = 0;
  while i<4 {
    let shift = (3 - i) * 8;
    bytes.push(((val >> shift) & 0xFF) as u8);
    i = i + 1;
  }
  bytes
}

pub fn bytes_to_i32(bytes:&Vec<u8>, off:usize) -> i32{
  let mut i = 0;
  let mut val:i32 = 0;
  while i<4 {
    let shift = (3 - i) * 8;
    val += ((bytes[i + off] as i32) & 0xFF) << shift;
    i = i + 1;
  }
  val
}


pub fn properties_to_bytes(props: HashMap<usize, DataProperty>) -> Vec<u8> {
  let mut bytes: Vec<u8> = Vec::new();
  for (key, val) in props {
    bytes.append(&mut val.to_bytes());
  }
  bytes
}

pub fn bytes_to_properties(bytes:Vec<u8>, off:usize, len:usize) -> HashMap<usize, DataProperty>{
  let mut map: HashMap<usize, DataProperty> = HashMap::new();
  let n = len - off;
  let mut i = off;
  while i<n {
    let mut dp:DataProperty = DataProperty::from_bytes(&bytes, i);
    map.insert(dp.id, dp);
    i = i + PROPERTY_SIZE as usize;
  }
  map
}

