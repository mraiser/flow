use serde_json::*;
//use std::fmt;

use crate::flowenv::*;
use crate::data::*;
use crate::dataobject::*;

pub struct DataArray {
  pub data_ref: usize,
}

impl DataArray {
  pub fn new(env:&mut FlowEnv) -> DataArray {
    let data_ref = &mut env.arrays.push(Vec::<Data>::new());
    return DataArray {
      data_ref: *data_ref,
    };
  }
  
  pub fn get(data_ref: usize, env:&mut FlowEnv) -> DataArray {
    let o = DataArray{
      data_ref: data_ref,
    };
    let _x = &mut env.arrays.incr(data_ref);
    o
  }
  
  pub fn from_json(value:Value, env:&mut FlowEnv) -> DataArray {
    let mut o = DataArray::new(env);
    
    for val in value.as_array().unwrap().iter() {
      if val.is_string(){ o.push_str(val.as_str().unwrap(), env); }
      else if val.is_boolean() { o.push_bool(val.as_bool().unwrap(), env); }
      else if val.is_i64() { o.push_i64(val.as_i64().unwrap(), env); }
      else if val.is_f64() { o.push_float(val.as_f64().unwrap(), env); }
      else if val.is_object() { o.push_object(DataObject::from_json(val.to_owned(), env), env); }
      else if val.is_array() { o.push_list(DataArray::from_json(val.to_owned(), env), env); }      
      else { println!("Unknown type {}", val) };
    }
      
    o
  }
  
  pub fn to_json(&self, env:&mut FlowEnv) -> Value {
    let mut val = Vec::<Value>::new();
    let mut id = 0;
    for old in self.objects(env) {
      if old.is_int() { val.push(json!(self.get_i64(id, env))); }
      else if old.is_float() { val.push(json!(self.get_f64(id, env))); }
      else if old.is_boolean() { val.push(json!(self.get_bool(id, env))); }
      else if old.is_string() { val.push(json!(self.get_string(id, env))); }
      else if old.is_object() { val.push(self.get_object(id, env).to_json(env)); }
      else if old.is_array() { val.push(self.get_array(id, env).to_json(env)); }
      else { val.push(json!(null)); }
      id = id + 1;
    }
    json!(val)
  }
  
  pub fn duplicate(&self, env:&mut FlowEnv) -> DataArray {
    let o = DataArray{
      data_ref: self.data_ref,
    };
    let _x = &mut env.arrays.incr(self.data_ref);
    o
  }
  
  pub fn shallow_copy(self, env:&mut FlowEnv) -> DataArray {
    let mut o = DataArray::new(env);
    for v in self.objects(env) {
      o.push_property(v.clone(), env);
    }
    o
  }

  pub fn deep_copy(&self, env:&mut FlowEnv) -> DataArray {
    let mut o = DataArray::new(env);
    let mut id = 0;
    for v in self.objects(env) {
      if v.is_object() {
        o.push_object(self.get_object(id, env).deep_copy(env), env);
      }
      else if v.is_array() {
        o.push_list(self.get_array(id, env).deep_copy(env), env);
      }
      else {
        o.push_property(v.clone(), env);
      }
      id = id + 1;
    }
    o
  }

  pub fn len(&self, env:&mut FlowEnv) -> usize {
    let heap = &mut env.arrays;
    let vec = heap.get(self.data_ref);
    vec.len()
  }

  pub fn get_property(&self, id:usize, env:&mut FlowEnv) -> Data {
    let heap = &mut env.arrays;
    let vec = heap.get(self.data_ref);
    let data = vec.get_mut(id).unwrap();
    data.clone()
  }
  
  pub fn get_string(&self, id:usize, env:&mut FlowEnv) -> String {
    self.get_property(id, env).string()
  }
  
  pub fn get_bool(&self, id:usize, env:&mut FlowEnv) -> bool {
    self.get_property(id, env).boolean()
  }
  
  pub fn get_i64(&self, id:usize, env:&mut FlowEnv) -> i64 {
    self.get_property(id, env).int()
  }
  
  pub fn get_f64(&self, id:usize, env:&mut FlowEnv) -> f64 {
    self.get_property(id, env).float()
  }

  pub fn get_array(&self, id:usize, env:&mut FlowEnv) -> DataArray {
    self.get_property(id, env).array(env)
  }

  pub fn get_object(&self, id:usize, env:&mut FlowEnv) -> DataObject {
    self.get_property(id, env).object(env)
  }

  pub fn push_property(&mut self, data:Data, env:&mut FlowEnv) {
    if let Data::DObject(i) = &data {
      env.objects.incr(*i);
    }
    else if let Data::DArray(i) = &data {
      env.arrays.incr(*i); 
    }
  
    let vec = env.arrays.get(self.data_ref);
    vec.push(data);
  }

  pub fn push_str(&mut self, val:&str, env:&mut FlowEnv) {
    self.push_property(Data::DString(val.to_string()), env);
  }
  
  pub fn push_bool(&mut self, val:bool, env:&mut FlowEnv) {
    self.push_property(Data::DBoolean(val), env);
  }
  
  pub fn push_i64(&mut self, val:i64, env:&mut FlowEnv) {
    self.push_property(Data::DInt(val), env);
  }
  
  pub fn push_float(&mut self, val:f64, env:&mut FlowEnv) {
    self.push_property(Data::DFloat(val), env);
  }

  pub fn push_object(&mut self, o:DataObject, env:&mut FlowEnv) {
    self.push_property(Data::DObject(o.data_ref), env);
  }
  
  pub fn push_list(&mut self, a:DataArray, env:&mut FlowEnv) {
    self.push_property(Data::DArray(a.data_ref), env);
  }
  
  // FIXME - add insert/set_...(index, value) function for all types
  
  pub fn remove_property(&mut self, id:usize, env:&mut FlowEnv) {
    let heap = &mut env.arrays;
    let vec = heap.get(self.data_ref);
    let old = vec.remove(id);
    if let Data::DObject(i) = &old {
      DataObject::delete(env, *i);
    }
    else if let Data::DArray(i) = &old {
      DataArray::delete(env, *i);
    }
  }
  
  pub fn delete(env:&mut FlowEnv, data_ref:usize) {
    let mut objects_to_kill = Vec::<usize>::new();
    let mut arrays_to_kill = Vec::<usize>::new();
    
    let heap = &mut env.arrays;
    
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
      DataObject::delete(env, i);
    }
    for i in arrays_to_kill {
      DataArray::delete(env, i);
    }
  }
  
  pub fn objects(&self, env:&mut FlowEnv) -> Vec<Data> {
    let heap = &mut env.arrays;
    let map = heap.get(self.data_ref);
    let mut vec = Vec::<Data>::new();
    for v in map {
      vec.push(v.clone());
    }
    vec
  }

  pub fn print_heap(env:&mut FlowEnv) {
    println!("array {:?}", &env.arrays);
  }
}

impl Drop for DataArray {
  fn drop(&mut self) {
    ADROP.get().write().unwrap().push(self.data_ref);
  }
}
/*
impl fmt::Debug for DataArray {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let env = &mut FLOWENV.get().write().unwrap();
    let val = self.to_json(env);
    write!(f, "{}", val)
  }
}
*/
