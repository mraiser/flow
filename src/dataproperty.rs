use std::collections::HashMap;
use crate::bytesref::*;
use crate::heap::*;

pub static TYPE_RAW:u8 = 0;
pub static TYPE_INT:u8 = 1;
pub static TYPE_LONG:u8 = 2;
pub static TYPE_FLOAT:u8 = 3;
pub static TYPE_BOOLEAN:u8 = 4;
pub static TYPE_STRING:u8 = 5;
pub static TYPE_DATA:u8 = 6;
pub static TYPE_LIST:u8 = 7;

#[derive(Debug)]
pub struct DataProperty {
  pub id: usize,
  pub typ: u8,
  pub value: BytesRef,
}

impl DataProperty {
  pub fn new(id: &str, typ: u8, value: BytesRef) -> DataProperty {
    DataProperty {
      id: BytesRef::lookup_prop(id),
      typ: typ,
      value: value,
    }
  }
}

