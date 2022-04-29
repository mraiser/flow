use serde::*;
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
  pub fn new(env:&mut FlowEnv) -> DataObject {
    let data_ref = &mut env.objects.push(HashMap::<String,Data>::new());
    return DataObject {
      data_ref: *data_ref,
    };
  }
  
  pub fn get(data_ref: usize, env:&mut FlowEnv) -> DataObject {
    let o = DataObject{
      data_ref: data_ref,
    };
    let _x = &mut env.objects.incr(data_ref);
    o
  }
  
  pub fn from_json(value:Value, env:&mut FlowEnv) -> DataObject {
    let mut o = DataObject::new(env);
    
    for (key, val) in value.as_object().unwrap().iter() {
      if val.is_string(){ o.put_str(key, val.as_str().unwrap(), env); }
      else if val.is_boolean() { o.put_bool(key, val.as_bool().unwrap(), env); }
      else if val.is_i64() { o.put_i64(key, val.as_i64().unwrap(), env); }
      else if val.is_f64() { o.put_float(key, val.as_f64().unwrap(), env); }
      else if val.is_object() { o.put_object(key, DataObject::from_json(val.to_owned(), env), env); }
      else if val.is_array() { o.put_list(key, DataArray::from_json(val.to_owned(), env), env); }      
      else if val.is_null() { o.put_null(key, env); }
      else { println!("Unknown type {}", val) };
    }
    o
  }
  
  pub fn to_json(&self, env:&mut FlowEnv) -> Value {
    let mut val = json!({});
    for (keystr,old) in self.objects(env) {
      if old.is_int() { val[keystr] = json!(self.get_i64(&keystr, env)); }
      else if old.is_float() { val[keystr] = json!(self.get_f64(&keystr, env)); }
      else if old.is_boolean() { val[keystr] = json!(self.get_bool(&keystr, env)); }
      else if old.is_string() { val[keystr] = json!(self.get_string(&keystr, env)); }
      else if old.is_object() { val[keystr] = self.get_object(&keystr, env).to_json(env); }
      else if old.is_array() { val[keystr] = self.get_array(&keystr, env).to_json(env); }
      else { val[keystr] = json!(null); }
    }
    val
  }
  
  pub fn duplicate(&self, env:&mut FlowEnv) -> DataObject {
    let o = DataObject{
      data_ref: self.data_ref,
    };
    let _x = &mut env.objects.incr(self.data_ref);
    o
  }
  
  pub fn shallow_copy(&self, env:&mut FlowEnv) -> DataObject {
    let mut o = DataObject::new(env);
    for (k,v) in self.objects(env) {
      o.set_property(&k, v.clone(), env);
    }
    o
  }

  pub fn deep_copy(&self, env:&mut FlowEnv) -> DataObject {
    let mut o = DataObject::new(env);
    for (key,v) in self.objects(env) {
      if v.is_object() {
        o.put_object(&key, self.get_object(&key, env).deep_copy(env), env);
      }
      else if v.is_array() {
        o.put_list(&key, self.get_array(&key, env).deep_copy(env), env);
      }
      else {
        o.set_property(&key, v.clone(), env);
      }
    }
    o
  }
  
  pub fn has(&self, key:&str, env:&mut FlowEnv) -> bool {
    let heap = &mut env.objects;
    let map = heap.get(self.data_ref);
    map.contains_key(key)
  }
  
  pub fn keys(self, env:&mut FlowEnv) -> Vec<String> {
    let mut vec = Vec::<String>::new();
    for (key, _val) in self.objects(env) {
      vec.push(key)
    }
    vec
  }
  
  pub fn get_property(&self, key:&str, env:&mut FlowEnv) -> Data {
    let heap = &mut env.objects;
    let map = heap.get(self.data_ref);
    let data = map.get(key).unwrap();
    data.clone()
  }
  
  pub fn get_string(&self, key:&str, env:&mut FlowEnv) -> String {
    self.get_property(key, env).string()
  }
  
  pub fn get_bool(&self, key:&str, env:&mut FlowEnv) -> bool {
    self.get_property(key, env).boolean()
  }
  
  pub fn get_i64(&self, key:&str, env:&mut FlowEnv) -> i64 {
    self.get_property(key, env).int()
  }
  
  pub fn get_f64(&self, key:&str, env:&mut FlowEnv) -> f64 {
    self.get_property(key, env).float()
  }
  
  pub fn get_object(&self, key:&str, env:&mut FlowEnv) -> DataObject {
    self.get_property(key, env).object(env)
  }
  
  pub fn get_array(&self, key:&str, env:&mut FlowEnv) -> DataArray {
    self.get_property(key, env).array(env)
  }
  
  pub fn remove_property(&mut self, key:&str, env:&mut FlowEnv) {
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
  
  pub fn set_property(&mut self, key:&str, data:Data, env:&mut FlowEnv) {
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
  
  pub fn put_str(&mut self, key:&str, val:&str, env:&mut FlowEnv) {
    self.set_property(key,Data::DString(val.to_string()), env);
  }
  
  pub fn put_bool(&mut self, key:&str, val:bool, env:&mut FlowEnv) {
    self.set_property(key,Data::DBoolean(val), env);
  }
  
  pub fn put_i64(&mut self, key:&str, val:i64, env:&mut FlowEnv) {
    self.set_property(key,Data::DInt(val), env);
  }
  
  pub fn put_float(&mut self, key:&str, val:f64, env:&mut FlowEnv) {
    self.set_property(key,Data::DFloat(val), env);
  }

  pub fn put_object(&mut self, key:&str, o:DataObject, env:&mut FlowEnv) {
    self.set_property(key, Data::DObject(o.data_ref), env);
  }
    
  pub fn put_list(&mut self, key:&str, a:DataArray, env:&mut FlowEnv) {
    self.set_property(key, Data::DArray(a.data_ref), env);
  }
  
  pub fn put_null(&mut self, key:&str, env:&mut FlowEnv) {
    self.set_property(key, Data::DNull, env);
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
  
  pub fn objects(&self, env:&mut FlowEnv) -> Vec<(String, Data)> {
    let heap = &mut env.objects;
    let map = heap.get(self.data_ref);
    let mut vec = Vec::<(String, Data)>::new();
    for (k,v) in map {
      vec.push((k.to_string(),v.clone()));
    }
    vec
  }
  
  pub fn print_heap(env:&mut FlowEnv) {
    println!("object {:?}", &mut env.objects);
  }
}

impl Drop for DataObject {
  fn drop(&mut self) {
    ODROP.get().write().unwrap().push(self.data_ref);
  }
}


// NOTE - Not used. Case requires it because runtime DataObject is stored in Options.
// WARNING - Will panic is called. Do not call.
impl<'de> Deserialize<'de> for DataObject {
    fn deserialize<D>(_deserializer: D) -> Result<DataObject, D::Error>
    where
      D: Deserializer<'de>,
    {
      let o = DataObject {
        data_ref: 0,
      };
      Ok(o)
    }
}

// NOTE - Not used. Case requires it because runtime DataObject is stored in Options.
impl Serialize for DataObject {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    serializer.serialize_none()
  }
}

/*
impl fmt::Debug for DataObject {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let env = &mut FLOWENV.get().write().unwrap();
    let val = self.to_json(env);
    write!(f, "{}", val)
  }
}
*/

