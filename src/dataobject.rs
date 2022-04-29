use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
//use std::fmt;

use crate::flowenv::*;
use crate::data::*;
use crate::dataarray::*;

#[derive(Debug, Default)]
pub struct DataObject {
  pub data_ref: usize,
}

impl DataObject {
  pub fn new() -> DataObject {
    let env = &mut FLOWENV.get().write().unwrap();
    let data_ref = &mut env.objects.push(HashMap::<String,Data>::new());
    return DataObject {
      data_ref: *data_ref,
    };
  }
  
  pub fn get(data_ref: usize) -> DataObject {
    let o = DataObject{
      data_ref: data_ref,
    };
    let env = &mut FLOWENV.get().write().unwrap();
    let _x = &mut env.objects.incr(data_ref);
    o
  }
  
  pub fn from_json(value:Value) -> DataObject {
    let mut o = DataObject::new();
    
    for (key, val) in value.as_object().unwrap().iter() {
      if val.is_string(){ o.put_str(key, val.as_str().unwrap()); }
      else if val.is_boolean() { o.put_bool(key, val.as_bool().unwrap()); }
      else if val.is_i64() { o.put_i64(key, val.as_i64().unwrap()); }
      else if val.is_f64() { o.put_float(key, val.as_f64().unwrap()); }
      else if val.is_object() { o.put_object(key, DataObject::from_json(val.to_owned())); }
      else if val.is_array() { o.put_list(key, DataArray::from_json(val.to_owned())); }      
      else if val.is_null() { o.put_null(key); }
      else { println!("Unknown type {}", val) };
    }
    o
  }
  
  pub fn to_json(&self) -> Value {
    let mut val = json!({});
    for (keystr,old) in self.objects() {
      if old.is_int() { val[keystr] = json!(self.get_i64(&keystr)); }
      else if old.is_float() { val[keystr] = json!(self.get_f64(&keystr)); }
      else if old.is_boolean() { val[keystr] = json!(self.get_bool(&keystr)); }
      else if old.is_string() { val[keystr] = json!(self.get_string(&keystr)); }
      else if old.is_object() { val[keystr] = self.get_object(&keystr).to_json(); }
      else if old.is_array() { val[keystr] = self.get_array(&keystr).to_json(); }
      else { val[keystr] = json!(null); }
    }
    val
  }
  
  pub fn duplicate(&self) -> DataObject {
    let o = DataObject{
      data_ref: self.data_ref,
    };
    let env = &mut FLOWENV.get().write().unwrap();
    let _x = &mut env.objects.incr(self.data_ref);
    o
  }
  
  pub fn shallow_copy(&self) -> DataObject {
    let mut o = DataObject::new();
    for (k,v) in self.objects() {
      o.set_property(&k, v.clone());
    }
    o
  }

  pub fn deep_copy(&self) -> DataObject {
    let mut o = DataObject::new();
    for (key,v) in self.objects() {
      if v.is_object() {
        o.put_object(&key, self.get_object(&key).deep_copy());
      }
      else if v.is_array() {
        o.put_list(&key, self.get_array(&key).deep_copy());
      }
      else {
        o.set_property(&key, v.clone());
      }
    }
    o
  }
  
  pub fn has(&self, key:&str) -> bool {
    let env = &mut FLOWENV.get().write().unwrap();
    let heap = &mut env.objects;
    let map = heap.get(self.data_ref);
    map.contains_key(key)
  }
  
  pub fn keys(self) -> Vec<String> {
    let mut vec = Vec::<String>::new();
    for (key, _val) in self.objects() {
      vec.push(key)
    }
    vec
  }
  
  pub fn get_property(&self, key:&str) -> Data {
    let env = &mut FLOWENV.get().write().unwrap();
    let heap = &mut env.objects;
    let map = heap.get(self.data_ref);
    let data = map.get(key).unwrap();
    data.clone()
  }
  
  pub fn get_string(&self, key:&str) -> String {
    self.get_property(key).string()
  }
  
  pub fn get_bool(&self, key:&str) -> bool {
    self.get_property(key).boolean()
  }
  
  pub fn get_i64(&self, key:&str) -> i64 {
    self.get_property(key).int()
  }
  
  pub fn get_f64(&self, key:&str) -> f64 {
    self.get_property(key).float()
  }
  
  pub fn get_object(&self, key:&str) -> DataObject {
    self.get_property(key).object()
  }
  
  pub fn get_array(&self, key:&str) -> DataArray {
    self.get_property(key).array()
  }
  
  pub fn remove_property(&mut self, key:&str) {
    let env = &mut FLOWENV.get().write().unwrap();
    let heap = &mut env.objects;
    let map = heap.get(self.data_ref);
    if let Some(old) = map.remove(key){
      if let Data::DObject(i) = &old {
        DataObject::delete(env, *i);
      }
      else if let Data::DArray(i) = &old {
        DataArray::delete(env, *i);
      }
    }
  }
  
  pub fn set_property(&mut self, key:&str, data:Data) {
    let env = &mut FLOWENV.get().write().unwrap();
    if let Data::DObject(i) = &data {
      let heap = &mut env.objects;
      heap.incr(*i); 
    }
    else if let Data::DArray(i) = &data {
      let aheap = &mut env.arrays;
      aheap.incr(*i);
    }
    
    let heap = &mut env.objects;
    let map = heap.get(self.data_ref);
    if let Some(old) = map.insert(key.to_string(),data){
      if let Data::DObject(i) = &old {
        DataObject::delete(env, *i);
      }
      else if let Data::DArray(i) = &old {
        DataArray::delete(env, *i);
      }
    }
  }
  
  pub fn put_str(&mut self, key:&str, val:&str) {
    self.set_property(key,Data::DString(val.to_string()));
  }
  
  pub fn put_bool(&mut self, key:&str, val:bool) {
    self.set_property(key,Data::DBoolean(val));
  }
  
  pub fn put_i64(&mut self, key:&str, val:i64) {
    self.set_property(key,Data::DInt(val));
  }
  
  pub fn put_float(&mut self, key:&str, val:f64) {
    self.set_property(key,Data::DFloat(val));
  }

  pub fn put_object(&mut self, key:&str, o:DataObject) {
    self.set_property(key, Data::DObject(o.data_ref));
  }
    
  pub fn put_list(&mut self, key:&str, a:DataArray) {
    self.set_property(key, Data::DArray(a.data_ref));
  }
  
  pub fn put_null(&mut self, key:&str) {
    self.set_property(key, Data::DNull);
  }
  
  pub fn delete(env:&mut FlowEnv, data_ref:usize) {
    let mut objects_to_kill = Vec::<usize>::new();
    let mut arrays_to_kill = Vec::<usize>::new();
    
    let heap = &mut env.objects;
    
    let n = heap.count(data_ref);
    if n == 1 {
      let map = heap.get(data_ref);
      for (_k,v) in map {
        if let Data::DObject(i) = v {
          objects_to_kill.push(*i);
        }
        else if let Data::DArray(i) = v {
          arrays_to_kill.push(*i);
        }
      }
    }
    heap.decr(data_ref);
    
    for i in objects_to_kill {
      DataObject::delete(env, i);
    }
    for i in arrays_to_kill {
      DataArray::delete(env, i);
    }
  }
  
  pub fn objects(&self) -> Vec<(String, Data)> {
    let env = &mut FLOWENV.get().write().unwrap();
    let heap = &mut env.objects;
    let map = heap.get(self.data_ref);
    let mut vec = Vec::<(String, Data)>::new();
    for (k,v) in map {
      vec.push((k.to_string(),v.clone()));
    }
    vec
  }
  
  pub fn print_heap() {
    let env = &mut FLOWENV.get().write().unwrap();
    println!("object {:?}", &mut env.objects);
  }
}

impl Drop for DataObject {
  fn drop(&mut self) {
    ODROP.get().write().unwrap().push(self.data_ref);
  }
}

/*
impl fmt::Debug for DataObject {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let env = &mut FLOWENV.get().write().unwrap();
    let val = self.to_json();
    write!(f, "{}", val)
  }
}
*/

