use std::collections::HashMap;
use crate::bytesref::*;
use crate::bytesutil::*;
use crate::heap::*;

pub static TYPE_RAW:u8 = 0;
pub static TYPE_INT:u8 = 1;
pub static TYPE_LONG:u8 = 2;
pub static TYPE_FLOAT:u8 = 3;
pub static TYPE_BOOLEAN:u8 = 4;
pub static TYPE_STRING:u8 = 5;
pub static TYPE_OBJECT:u8 = 6;
pub static TYPE_LIST:u8 = 7;

pub static PROPERTY_SIZE:u8 = 33;

#[derive(Debug, Copy, Clone)]
pub struct DataProperty {
  pub id: usize,
  pub typ: u8,
  pub byte_ref: usize,
  pub off: usize,
  pub len:usize,
}

impl DataProperty {
  pub fn new(id: &str, typ: u8, value: BytesRef) -> DataProperty {
    DataProperty {
      id: BytesRef::lookup_prop(id),
      typ: typ,
      byte_ref: value.byte_ref,
      off: value.off,
      len: value.len,
    }
  }
  
  pub fn from_bytes(bytes:&Vec<u8>, off:usize) -> DataProperty {
    let id:usize = bytes_to_i64(bytes, off) as usize;
    let typ:u8 = bytes[off + 8];
    let byte_ref:usize = bytes_to_i64(bytes, off+9) as usize;
    let off:usize = bytes_to_i64(bytes, off+17) as usize;
    let len:usize = bytes_to_i64(bytes, off+25) as usize;
    DataProperty {
      id: id,
      typ: typ,
      byte_ref: byte_ref,
      off: off,
      len: len,
    }
  }
  
  pub fn to_bytes(&self) -> Vec<u8>{
    let mut bytes: Vec<u8> = Vec::new();
    bytes.append(&mut i64_to_bytes(self.id as i64));
    bytes.append(&mut vec![self.typ]);
    bytes.append(&mut i64_to_bytes(self.byte_ref as i64));
    bytes.append(&mut i64_to_bytes(self.off as i64));
    bytes.append(&mut i64_to_bytes(self.len as i64));
    bytes
  }
}

