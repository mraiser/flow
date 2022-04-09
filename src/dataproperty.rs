use crate::bytesref::*;
use crate::bytesutil::*;

//pub static TYPE_RAW:u8 = 0;
pub static TYPE_NULL:u8 = 1;
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
  pub fn new(id: usize, typ: u8, value: BytesRef) -> DataProperty {
    DataProperty {
      id: id,
      typ: typ,
      byte_ref: value.byte_ref,
      off: value.off,
      len: value.len,
    }
  }
  
  pub fn is_number(&self) -> bool {
    self.is_f64() || self.is_i64()
  }
  
  pub fn is_i64(&self) -> bool {
    self.typ == TYPE_LONG
  }
  
  pub fn is_f64(&self) -> bool {
    self.typ == TYPE_FLOAT
  }
  
  pub fn is_string(&self) -> bool {
    self.typ == TYPE_STRING
  }
  
  pub fn is_bool(&self) -> bool {
    self.typ == TYPE_BOOLEAN
  }
  
  pub fn to_bytes_ref(&self) -> BytesRef {
    BytesRef::get(self.byte_ref, self.off, self.len)
  }
  
  pub fn as_i64(&self) -> i64 {
    self.to_bytes_ref().as_i64()
  }
  
  pub fn as_f64(&self) -> f64 {
    self.to_bytes_ref().as_f64()
  }
  
  pub fn as_bool(&self) -> bool {
    self.to_bytes_ref().as_bool()
  }
  
  pub fn as_string(&self) -> String {
    self.to_bytes_ref().as_string()
  }
  
  pub fn from_bytes(bytes:&Vec<u8>, off:usize) -> DataProperty {
    let id:usize = bytes_to_i64(bytes, off) as usize;
    let typ:u8 = bytes[off + 8];
    let byte_ref:usize = bytes_to_i64(bytes, off+9) as usize;
    let off2:usize = bytes_to_i64(bytes, off+17) as usize;
    let len:usize = bytes_to_i64(bytes, off+25) as usize;
    DataProperty {
      id: id,
      typ: typ,
      byte_ref: byte_ref,
      off: off2,
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

