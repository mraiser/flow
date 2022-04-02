use serde_json::*;
use std::collections::HashMap;

use crate::bytesref::*;
use crate::dataproperty::*;

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
      else if val.is_array() { o.put_list(key, val.to_owned()); }      
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

//  pub fn bytes(&mut self) -> BytesRef {
//    let mut bytes:BytesRef = BytesRef::get(self.byte_ref, 0, 24);
//    bytes.from_handle()
    //BytesRef::get(bytes.byte_ref, bytes.off, bytes.len)
//  }
  
  pub fn get_property(&self, key:&str) -> DataProperty {
    let mut handle:BytesRef = BytesRef::get(self.byte_ref, 0, 24);
    let mut bytes = handle.from_handle();
    let mut props = bytes.as_properties();
    let id = BytesRef::lookup_prop(key);
    props.get(&id).unwrap().clone()
  }
  
  pub fn get_i64(&self, key:&str) -> i64 {
    let dp = self.get_property(key);
    let br = BytesRef::get(dp.byte_ref, dp.off, dp.len);
    br.as_i64()
  }
  
  pub fn set_property(&mut self, key:&str, typ:u8, bytesref:BytesRef) {
    // FIXME - Not thread safe. Call should be synchronized
    bytesref.incr();

    let mut handle:BytesRef = BytesRef::get(self.byte_ref, 0, 24);
    let mut bytes = handle.from_handle();
    
    let dp = DataProperty::new(key, typ, bytesref);
    let id = dp.id;

    //println!("Old property list {:?}", bytes.len);    
    let mut props = bytes.as_properties();
    if let Some(old) = props.insert(id, dp){
      BytesRef::get(old.byte_ref, old.off, old.len).decr();
      if old.typ == TYPE_OBJECT { // FIXME - Need to handle lists, too
        let mut o = DataObject {
          byte_ref: old.byte_ref,
        };
      }
    }
    let nubytes = BytesRef::properties_to_bytes(props);
    let n = nubytes.len();
    bytes.len = n;
    //println!("New property list {:?}", nubytes.len());
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
/*  
  fn incr_all(&self) {
    let mut handle:BytesRef = BytesRef::get(self.byte_ref, 0, 24);
    let mut bytes = handle.from_handle();
    let mut props = bytes.as_properties();
    for (key, val) in bytes.as_properties().iter() {
      println!("Inflating {}", val.byte_ref);
      BytesRef::get(val.byte_ref, val.off, val.len).incr();
    }
  }
*/  
  pub fn put_object(&mut self, key:&str, o:DataObject) {
//    let mut o = DataObject::from_json(val);
//    o.incr_all();
    let mut handle = BytesRef::get(o.byte_ref, 0, 24);
    handle.from_handle().incr();
    self.set_property(key, TYPE_OBJECT, handle);
  }
  
  pub fn put_list(&mut self, key:&str, val:Value) {
    // FIXME - Implement Lists
    let ba = BytesRef::from_bool(true);
    //let mut o = DataList::from_json(val);
    //let ba = o.bytes.duplicate();
    self.set_property(key, TYPE_LIST, ba);
  }
}

impl Drop for DataObject {
  fn drop(&mut self) {
    let mut handle = BytesRef::get(self.byte_ref, 0, 24);
    let n = handle.count();
    let mut bytes = handle.from_handle();
    //println!("Dropping data object {:?}, count: {}", self.byte_ref, n);
    let mut vec = Vec::<DataObject>::new();
    if n == 2 {
      for (key, old) in bytes.as_properties().iter() {
        //println!("Trying to drop {}", old.byte_ref);
        let mut ba = BytesRef::get(old.byte_ref, old.off, old.len);
        //ba.decr();
        if old.typ == TYPE_OBJECT { // FIXME - Need to handle lists, too
          {
            //println!("pre-drop count for {} is {}", ba.byte_ref, ba.count());
            let mut o = DataObject {
              byte_ref: ba.byte_ref,
            };
            vec.push(o);
          }
        }
        else { ba.decr(); }
      }
    }
    //println!("Bye-bye handle {}", handle.byte_ref);
    handle.decr();
    //println!("Bye-bye data {}", bytes.byte_ref);
    bytes.decr();
  }
}
