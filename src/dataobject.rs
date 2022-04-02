use serde_json::*;
use std::collections::HashMap;

use crate::bytesref::*;
use crate::bytesutil::*;
use crate::dataproperty::*;
use crate::dataarray::*;

#[derive(Debug)]
pub struct DataObject {
  pub byte_ref: usize,
}

impl DataObject {
  pub fn from_json(value:Value) -> DataObject {
    let mut bytes: Vec<u8> = Vec::<u8>::new();
    let mut ba = BytesRef::push(bytes);
    let mut ba = ba.to_handle();
    ba.incr();
    let mut o = DataObject {
      byte_ref: ba.byte_ref,
    };
    
    for (key, val) in value.as_object().unwrap().iter() {
      if val.is_string(){ o.put_str(key, val.as_str().unwrap()); }
      else if val.is_boolean() { o.put_bool(key, val.as_bool().unwrap()); }
      else if val.is_i64() { o.put_i64(key, val.as_i64().unwrap()); }
      else if val.is_f64() { o.put_float(key, val.as_f64().unwrap()); }
      else if val.is_object() { o.put_object(key, DataObject::from_json(val.to_owned())); }
      else if val.is_array() { o.put_list(key, DataArray::from_json(val.to_owned())); }      
      else { println!("Unknown type {}", val) };
      //println!("{} / {}", key, val);
    }
      
    o
  }

  pub fn lookup_prop(&self, name: &str) -> usize {
    BytesRef::lookup_prop(name)
  }
  
  pub fn lookup_prop_string(&self, i: usize) -> String {
    BytesRef::lookup_prop_string(i)
  }  

  pub fn get_property(&self, key:&str) -> DataProperty {
    let mut handle:BytesRef = BytesRef::get(self.byte_ref, 0, 24);
    let mut bytes = handle.from_handle();
    let mut props = bytes.as_propertymap();
    let id = BytesRef::lookup_prop(key);
    props.get(&id).unwrap().clone()
  }
  
  pub fn get_string(&self, key:&str) -> String {
    let dp = self.get_property(key);
    let br = BytesRef::get(dp.byte_ref, dp.off, dp.len);
    br.as_string()
  }
  
  pub fn get_bool(&self, key:&str) -> bool {
    let dp = self.get_property(key);
    let br = BytesRef::get(dp.byte_ref, dp.off, dp.len);
    br.as_bool()
  }
  
  pub fn get_i64(&self, key:&str) -> i64 {
    let dp = self.get_property(key);
    let br = BytesRef::get(dp.byte_ref, dp.off, dp.len);
    br.as_i64()
  }
  
  pub fn get_f64(&self, key:&str) -> f64 {
    let dp = self.get_property(key);
    let br = BytesRef::get(dp.byte_ref, dp.off, dp.len);
    br.as_f64()
  }
  
  pub fn set_property(&mut self, key:&str, typ:u8, bytesref:BytesRef) {
    // FIXME - Not thread safe. Call should be synchronized
    bytesref.incr();

    let mut handle:BytesRef = BytesRef::get(self.byte_ref, 0, 24);
    let mut bytes = handle.from_handle();
    
    let dp = DataProperty::new(self.lookup_prop(key), typ, bytesref);
    let id = dp.id;

    let mut props = bytes.as_propertymap();
    if let Some(old) = props.insert(id, dp){
      if old.typ == TYPE_OBJECT {
        let mut o = DataObject {
          byte_ref: old.byte_ref,
        };
      }
      else if old.typ == TYPE_LIST {
        let mut o = DataArray {
          byte_ref: old.byte_ref,
        };
      }
      else {
        BytesRef::get(old.byte_ref, old.off, old.len).decr();
      }
    }
    let nubytes = propertymap_to_bytes(props);
    let n = nubytes.len();
    bytes.len = n;
    bytes.swap(nubytes);
    handle.swap(bytes.to_handle_bytes());
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
    self.set_property(key, TYPE_LONG, ba);
  }
  
  pub fn put_float(&mut self, key:&str, val:f64) {
    let ba = BytesRef::from_f64(val);
    self.set_property(key, TYPE_FLOAT, ba);
  }

  pub fn put_object(&mut self, key:&str, o:DataObject) {
    let mut handle = BytesRef::get(o.byte_ref, 0, 24);
    handle.from_handle().incr();
    self.set_property(key, TYPE_OBJECT, handle);
  }
  
  pub fn put_list(&mut self, key:&str, a:DataArray) {
    let mut handle = BytesRef::get(a.byte_ref, 0, 24);
    handle.from_handle().incr();
    self.set_property(key, TYPE_LIST, handle);
//    let ba = BytesRef::from_bool(false);
//    self.set_property(key, TYPE_BOOLEAN, ba);
  }
}

impl Drop for DataObject {
  fn drop(&mut self) {
    let mut handle = BytesRef::get(self.byte_ref, 0, 24);
    let n = handle.count();
    let mut bytes = handle.from_handle();
    let mut objects_to_kill = Vec::<DataObject>::new();
    let mut arrays_to_kill = Vec::<DataArray>::new();
    if n == 2 {
      for (key, old) in bytes.as_propertymap().iter() {
        let mut ba = BytesRef::get(old.byte_ref, old.off, old.len);
        if old.typ == TYPE_OBJECT {
          {
            let mut o = DataObject {
              byte_ref: ba.byte_ref,
            };
            objects_to_kill.push(o);
          }
        }
        else if old.typ == TYPE_LIST {
          {
            let mut a = DataArray {
              byte_ref: ba.byte_ref,
            };
            arrays_to_kill.push(a);
          }
        }
        else { ba.decr(); }
      }
    }
    handle.decr();
    bytes.decr();
  }
}
