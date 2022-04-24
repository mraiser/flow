use serde_json::*;
use std::collections::HashMap;
use once_cell::sync::Lazy; // 1.3.1
use std::sync::Mutex;
use std::fmt;

use crate::heap::*;
use crate::data::*;
use crate::dataobject::*;

static HEAP: Lazy<Mutex<Heap<Vec<Data>>>> = Lazy::new(|| Mutex::new(Heap::new()));

pub struct DataArray {
  pub data_ref: usize,
}

impl DataArray {
  pub fn new() -> DataArray {
    let data_ref = HEAP.lock().unwrap().push(Vec::<Data>::new());
    return DataArray {
      data_ref: data_ref,
    };
  }
  
  pub fn get(data_ref: usize) -> DataArray {
    let mut o = DataArray{
      data_ref: data_ref,
    };
    DataArray::incref(data_ref);
    o
  }
  
  pub fn incref(data_ref:usize) {
    HEAP.lock().unwrap().incr(data_ref);
  }
  
  pub fn from_json(value:Value) -> DataArray {
    let mut o = DataArray::new();
    
    for val in value.as_array().unwrap().iter() {
      if val.is_string(){ o.push_str(val.as_str().unwrap()); }
      else if val.is_boolean() { o.push_bool(val.as_bool().unwrap()); }
      else if val.is_i64() { o.push_i64(val.as_i64().unwrap()); }
      else if val.is_f64() { o.push_float(val.as_f64().unwrap()); }
      else if val.is_object() { o.push_object(DataObject::from_json(val.to_owned())); }
      else if val.is_array() { o.push_list(DataArray::from_json(val.to_owned())); }      
      else { println!("Unknown type {}", val) };
    }
      
    o
  }
  
  pub fn to_json(&self) -> Value {
    let mut val = Vec::<Value>::new();
    let mut id = 0;
    for old in self.duplicate() {
      if old.is_int() { val.push(json!(self.get_i64(id))); }
      else if old.is_float() { val.push(json!(self.get_f64(id))); }
      else if old.is_boolean() { val.push(json!(self.get_bool(id))); }
      else if old.is_string() { val.push(json!(self.get_string(id))); }
      else if old.is_object() { val.push(self.get_object(id).to_json()); }
      else if old.is_array() { val.push(self.get_array(id).to_json()); }
      else { val.push(json!(null)); }
      id = id + 1;
    }
    json!(val)
  }
  
  pub fn duplicate(&self) -> DataArray {
    let mut o = DataArray{
      data_ref: self.data_ref,
    };
    DataArray::incref(self.data_ref);
    o
  }
  
  pub fn shallow_copy(self) -> DataArray {
    let mut o = DataArray::new();
    for v in self {
      o.push_property(v.clone());
    }
    o
  }

  pub fn deep_copy(&self) -> DataArray {
    let mut o = DataArray::new();
    let mut id = 0;
    for v in self.duplicate() {
      if v.is_object() {
        o.push_object(self.get_object(id).deep_copy());
      }
      else if v.is_array() {
        o.push_list(self.get_array(id).deep_copy());
      }
      else {
        o.push_property(v.clone());
      }
      id = id + 1;
    }
    o
  }

  pub fn len(&self) -> usize {
    let mut heap = HEAP.lock().unwrap();
    let vec = heap.get(self.data_ref);
    vec.len()
  }

  pub fn get_property(&self, id:usize) -> Data {
    let mut heap = HEAP.lock().unwrap();
    let vec = heap.get(self.data_ref);
    let data = vec.get_mut(id).unwrap();
    data.clone()
  }
  
  pub fn get_string(&self, id:usize) -> String {
    self.get_property(id).string()
  }
  
  pub fn get_bool(&self, id:usize) -> bool {
    self.get_property(id).boolean()
  }
  
  pub fn get_i64(&self, id:usize) -> i64 {
    self.get_property(id).int()
  }
  
  pub fn get_f64(&self, id:usize) -> f64 {
    self.get_property(id).float()
  }

  pub fn get_array(&self, id:usize) -> DataArray {
    self.get_property(id).array()
  }

  pub fn get_object(&self, id:usize) -> DataObject {
    self.get_property(id).object()
  }

  pub fn push_property(&mut self, data:Data) {
    let mut heap = HEAP.lock().unwrap();

    if let Data::DObject(i) = &data {
      DataObject::incref(*i); 
    }
    else if let Data::DArray(i) = &data {
      heap.incr(*i); 
    }
  
    let vec = heap.get(self.data_ref);
    vec.push(data);
  }

  pub fn push_str(&mut self, val:&str) {
    self.push_property(Data::DString(val.to_string()));
  }
  
  pub fn push_bool(&mut self, val:bool) {
    self.push_property(Data::DBoolean(val));
  }
  
  pub fn push_i64(&mut self, val:i64) {
    self.push_property(Data::DInt(val));
  }
  
  pub fn push_float(&mut self, val:f64) {
    self.push_property(Data::DFloat(val));
  }

  pub fn push_object(&mut self, o:DataObject) {
    self.push_property(Data::DObject(o.data_ref));
  }
  
  pub fn push_list(&mut self, a:DataArray) {
    self.push_property(Data::DArray(a.data_ref));
  }
  
  // FIXME - add insert/set_...(index, value) function for all types
  
  pub fn remove_property(&mut self, id:usize) {
    let mut objects_to_kill = Vec::<DataObject>::new();
    let mut arrays_to_kill = Vec::<DataArray>::new();
    
    let mut heap = HEAP.lock().unwrap();
    let vec = heap.get(self.data_ref);
    let old = vec.remove(id);
    if let Data::DObject(i) = &old {
      let x = DataObject { data_ref: *i, };
      objects_to_kill.push(x);
    }
    else if let Data::DArray(i) = &old {
      let x = DataArray { data_ref: *i, };
      arrays_to_kill.push(x);
    }
  }
  
  pub fn delete_object(oheap:&mut Heap<HashMap<String,Data>>, data_ref:usize) {
    let mut heap = HEAP.lock().unwrap();
    DataObject::delete(oheap, data_ref, &mut heap);
  }
  
  pub fn delete(heap:&mut Heap<Vec<Data>>, data_ref:usize, oheap:&mut Heap<HashMap<String,Data>>) {
    let mut objects_to_kill = Vec::<usize>::new();
    let mut arrays_to_kill = Vec::<usize>::new();
    
    let n = heap.count(data_ref);
    if n == 1 {
      let map = heap.get(data_ref);
      for v in map {
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
      DataObject::delete(oheap, i, heap);
    }
    for i in arrays_to_kill {
      DataArray::delete(heap, i, oheap);
    }
  }

  pub fn print_heap() {
    println!("{:?}", HEAP);
  }
}

impl IntoIterator for DataArray {
  type Item = Data;
  type IntoIter = DataArrayIterator;

  fn into_iter(self) -> Self::IntoIter {
    let mut heap = HEAP.lock().unwrap();
    let map = heap.get(self.data_ref);
    let mut vec = Vec::<Data>::new();
    for v in map {
      vec.push(v.clone());
    }
    DataArrayIterator {
      list: vec,
      index: 0,
    }
  }
}

pub struct DataArrayIterator {
  list: Vec<Data>,
  index: usize,
}

impl Iterator for DataArrayIterator {
  type Item = Data;
  fn next(&mut self) -> Option<Data> {
    let v = self.list.get(self.index)?;
    self.index += 1;
    Some(v.clone())
  }
}
  
impl Drop for DataArray {
  fn drop(&mut self) {
    let mut heap = HEAP.lock().unwrap();
    DataObject::delete_array(&mut heap, self.data_ref);
  }
}

impl fmt::Debug for DataArray {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let val = self.to_json();
    write!(f, "{}", val)
  }
}

