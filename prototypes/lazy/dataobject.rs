use serde_json::*;
use std::collections::HashMap;
use once_cell::sync::Lazy; // 1.3.1
use std::sync::Mutex;
use std::fmt;

use crate::heap::*;
use crate::data::*;
use crate::dataarray::*;

static HEAP: Lazy<Mutex<Heap<HashMap<String,Data>>>> = Lazy::new(|| Mutex::new(Heap::new()));

pub struct DataObject {
  pub data_ref: usize,
}

impl DataObject {
  pub fn new() -> DataObject {
    let data_ref = HEAP.lock().unwrap().push(HashMap::<String,Data>::new());
    return DataObject {
      data_ref: data_ref,
    };
  }
  
  pub fn get(data_ref: usize) -> DataObject {
    let mut o = DataObject{
      data_ref: data_ref,
    };
    o.incr();
    o
  }
  
  pub fn incr(&mut self) {
    HEAP.lock().unwrap().incr(self.data_ref);
  }
  
  pub fn decr(&mut self) {
    HEAP.lock().unwrap().decr(self.data_ref);
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
    for (keystr,old) in self.duplicate() {
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
    let mut o = DataObject{
      data_ref: self.data_ref,
    };
    o.incr();
    o
  }
  
  pub fn shallow_copy(&self) -> DataObject {
    let mut o = DataObject::new();
    for (k,v) in self.duplicate() {
      o.set_property(&k, v.clone());
    }
    o
  }

  pub fn deep_copy(&self) -> DataObject {
    let mut o = DataObject::new();
    for (key,v) in self.duplicate() {
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
    let mut heap = HEAP.lock().unwrap();
    let map = heap.get(self.data_ref);
    map.contains_key(key)
  }
  
  pub fn keys(self) -> Vec<String> {
    let mut vec = Vec::<String>::new();
    for (key, _val) in self {
      vec.push(key)
    }
    vec
  }
  
  pub fn get_property(&self, key:&str) -> Data {
    let mut heap = HEAP.lock().unwrap();
    let map = heap.get(self.data_ref);
    let data = map.get_mut(key).unwrap();
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
    let mut objects_to_kill = Vec::<DataObject>::new();
    let mut arrays_to_kill = Vec::<DataArray>::new();
    
    let mut heap = HEAP.lock().unwrap();
    let map = heap.get(self.data_ref);
    if let Some(old) = map.remove(key){
      if let Data::DObject(i) = &old {
        let x = DataObject { data_ref: *i, };
        objects_to_kill.push(x);
      }
      else if let Data::DArray(i) = &old {
        let x = DataArray { data_ref: *i, };
        arrays_to_kill.push(x);
      }
    }
  }
  
  pub fn set_property(&mut self, key:&str, data:Data) {
    let mut objects_to_kill = Vec::<DataObject>::new();
    let mut arrays_to_kill = Vec::<DataArray>::new();
    
    if data.is_object() {
      data.object().incr(); 
    }
    else if data.is_array() {
      data.array().incr(); 
    }
  
    let mut heap = HEAP.lock().unwrap();
    let map = heap.get(self.data_ref);
    if let Some(old) = map.insert(key.to_string(),data){
      if let Data::DObject(i) = &old {
        let x = DataObject { data_ref: *i, };
        objects_to_kill.push(x);
      }
      else if let Data::DArray(i) = &old {
        let x = DataArray { data_ref: *i, };
        arrays_to_kill.push(x);
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

  pub fn print_heap() {
    println!("{:?}", HEAP);
  }
}

impl IntoIterator for DataObject {
  type Item = (String, Data);
  type IntoIter = DataObjectIterator;

  fn into_iter(self) -> Self::IntoIter {
    let mut heap = HEAP.lock().unwrap();
    let map = heap.get(self.data_ref);
    let mut vec = Vec::<(String, Data)>::new();
    for (k,v) in map {
      vec.push((k.to_string(),v.clone()));
    }
    DataObjectIterator {
      list: vec,
      index: 0,
    }
  }
}

pub struct DataObjectIterator {
  list: Vec<(String, Data)>,
  index: usize,
}

impl Iterator for DataObjectIterator {
  type Item = (String, Data);
  fn next(&mut self) -> Option<(String,Data)> {
    let (k,v) = &self.list.get(self.index)?;
    self.index += 1;
    Some((k.to_string(),v.clone()))
  }
}

impl Drop for DataObject {
  fn drop(&mut self) {
    let mut objects_to_kill = Vec::<DataObject>::new();
    let mut arrays_to_kill = Vec::<DataArray>::new();
    
    let n = HEAP.lock().unwrap().count(self.data_ref);
    if n == 1 {
      let dup = self.duplicate();
      for (_k,v) in dup {
        if let Data::DObject(i) = v {
          let x = DataObject { data_ref: i, };
          objects_to_kill.push(x);
        }
        else if let Data::DArray(i) = v {
          let x = DataArray { data_ref: i, };
          arrays_to_kill.push(x);
        }
      }
    }
    self.decr();
  }
}

impl fmt::Debug for DataObject {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let val = self.to_json();
    write!(f, "{}", val)
  }
}

#[test]
fn verify_test() {
  let mut o = DataObject::new();
  o.put_i64("i", 1);
  assert_eq!(o.get_i64("i"), 1);
  o.put_float("j", 1.2);
  assert_eq!(o.get_f64("j"), 1.2);
}


