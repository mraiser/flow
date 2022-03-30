use serde_json::*;
use std::collections::HashMap;

use crate::bytesref::*;
use crate::dataproperty::*;

#[derive(Debug)]
pub struct DataObject {
  pub props: HashMap<usize, DataProperty>,
}

impl DataObject {
  pub fn from_json(value:Value) -> DataObject {
    let mut o = DataObject {
      props: HashMap::new(),
    };
    
    for (key, val) in value.as_object().unwrap().iter() {
      if val.is_string(){ o.put_str(key, val.as_str().unwrap()); }
      else if val.is_boolean() { o.put_bool(key, val.as_bool().unwrap()); }
      else if val.is_i64() { o.put_i64(key, val.as_i64().unwrap()); }
      else if val.is_f64() { o.put_float(key, val.as_f64().unwrap()); }
      //else if val.is_object() { o.put_object(key, val); }
      else { println!("Unknown type {}", val) };
      //println!("{} / {}", key, val);
    }
      
    o
  }

  pub fn lookup_prop(name: &str) -> usize {
    BytesRef::lookup_prop(name)
  }
  
  pub fn lookup_prop_string(&self, i: usize) -> String {
    BytesRef::lookup_prop_string(i)
  }  
  
  pub fn set_property(&mut self, key:&str, typ:u8, bytesref:BytesRef) {
    let dp = DataProperty::new(key, typ, bytesref);
    let id = dp.id.to_owned();
    self.props.insert(id, dp);
  }
  
  pub fn put_str(&mut self, key:&str, val:&str) {
    let ba = BytesRef::from_str(val);
    self.set_property(key, TYPE_STRING, ba);
  }
  
  pub fn put_bool(&mut self, key:&str, val:bool) {
    let ba = BytesRef::from_bool(val);
    self.set_property(key, TYPE_BOOLEAN, ba);
  }
  
  pub fn put_i64(&mut self, key:&str, val:i64) {
    let ba = BytesRef::from_i64(val);
    self.set_property(key, TYPE_INT, ba);
  }
  
  pub fn put_float(&mut self, key:&str, val:f64) {
    let ba = BytesRef::from_f64(val);
    self.set_property(key, TYPE_FLOAT, ba);
  }
}

