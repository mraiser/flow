use serde_json::*;
use std::fmt;

use crate::bytesref::*;
use crate::bytesutil::*;
use crate::dataproperty::*;
use crate::dataarray::*;

pub struct DataObject {
  pub byte_ref: usize,
}

impl DataObject {
  pub fn new() -> DataObject {
    let bytes: Vec<u8> = Vec::<u8>::new();
    let ba = BytesRef::push(bytes);
    let ba = ba.to_handle();
    ba.incr();
    DataObject {
      byte_ref: ba.byte_ref,
    }
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
    for old in self {
      let keystr = &self.lookup_prop_string(old.id);
      if old.typ == TYPE_LONG { val[keystr] = json!(self.get_i64(keystr)); }
      else if old.typ == TYPE_FLOAT { val[keystr] = json!(self.get_f64(keystr)); }
      else if old.typ == TYPE_BOOLEAN { val[keystr] = json!(self.get_bool(keystr)); }
      else if old.typ == TYPE_STRING { val[keystr] = json!(self.get_string(keystr)); }
      else if old.typ == TYPE_OBJECT { val[keystr] = self.get_object(keystr).to_json(); }
      else if old.typ == TYPE_LIST { val[keystr] = self.get_array(keystr).to_json(); }
      else { val[keystr] = json!(null); }
    }
    val
  }
  
  pub fn duplicate(&self) -> DataObject {
    let mut handle:BytesRef = BytesRef::get(self.byte_ref, 0, 24);
    let bytes = handle.from_handle();
    handle.incr();
    bytes.incr();
    DataObject {
      byte_ref: self.byte_ref,
    }
  }
  
  pub fn shallow_copy(&self) -> DataObject {
    let mut o = DataObject::new();
    for dp in self {
      o.set_property(&self.lookup_prop_string(dp.id), dp.typ, dp.to_bytes_ref());
    }
    o
  }

  pub fn deep_copy(&self) -> DataObject {
    let mut o = DataObject::new();
    for dp in self {
      let key = &self.lookup_prop_string(dp.id);
      if dp.typ == TYPE_OBJECT {
        o.put_object(key, self.get_object(key).deep_copy());
      }
      else if dp.typ == TYPE_LIST {
        o.put_list(key, self.get_array(key).deep_copy());
      }
      else {
        o.set_property(key, dp.typ, dp.to_bytes_ref());
      }
    }
    o
  }

  pub fn lookup_prop(&self, name: &str) -> usize {
    BytesRef::lookup_prop(name)
  }
  
  pub fn lookup_prop_string(&self, i: usize) -> String {
    BytesRef::lookup_prop_string(i)
  }  
  
  pub fn has(&self, key:&str) -> bool {
    let mut handle:BytesRef = BytesRef::get(self.byte_ref, 0, 24);
    let bytes = handle.from_handle();
    let props = bytes.as_propertymap();
    let id = self.lookup_prop(key);
    if let Some(_dp) = props.get(&id) {
      return true;
    }
    false
  }
  
  pub fn keys(&self) -> Vec<String> {
    let mut vec = Vec::<String>::new();
    for dp in self {
      let key = self.lookup_prop_string(dp.id);
      vec.push(key)
    }
    vec
  }
  
  pub fn get_property(&self, key:&str) -> DataProperty {
    let mut handle:BytesRef = BytesRef::get(self.byte_ref, 0, 24);
    let bytes = handle.from_handle();
    let props = bytes.as_propertymap();
    let id =self.lookup_prop(key);
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
  
  pub fn get_object(&self, key:&str) -> DataObject {
    let dp = self.get_property(key);
    let mut br = BytesRef::get(dp.byte_ref, dp.off, dp.len);
    br.incr();
    br.from_handle().incr();
    DataObject { byte_ref: br.byte_ref, }
  }
  
  pub fn get_array(&self, key:&str) -> DataArray {
    let dp = self.get_property(key);
    let mut br = BytesRef::get(dp.byte_ref, dp.off, dp.len);
    br.incr();
    br.from_handle().incr();
    DataArray { byte_ref: br.byte_ref, }
  }
  
  pub fn remove_property(&mut self, key:&str) {
    // FIXME - Not thread safe. Call should be synchronized
    let mut handle:BytesRef = BytesRef::get(self.byte_ref, 0, 24);
    let mut bytes = handle.from_handle();
    let mut props = bytes.as_propertymap();
    
    let dp = props.remove(&self.lookup_prop(key)).unwrap();
    if dp.typ == TYPE_OBJECT {
      let _o = DataObject {
        byte_ref: dp.byte_ref,
      };
    }
    else if dp.typ == TYPE_LIST {
      let _o = DataArray {
        byte_ref: dp.byte_ref,
      };
    }
    
    let nubytes = propertymap_to_bytes(props);
    let n = nubytes.len();
    bytes.len = n;
    bytes.swap(nubytes);
    handle.swap(bytes.to_handle_bytes());
  }
  
  pub fn set_property(&mut self, key:&str, typ:u8, mut bytesref:BytesRef) {
    // FIXME - Not thread safe. Call should be synchronized
    bytesref.incr();
    if typ == TYPE_OBJECT || typ == TYPE_LIST {
      bytesref.from_handle().incr();
    }

    let mut handle:BytesRef = BytesRef::get(self.byte_ref, 0, 24);
    let mut bytes = handle.from_handle();
    
    let dp = DataProperty::new(self.lookup_prop(key), typ, bytesref);
    let id = dp.id;

    let mut props = bytes.as_propertymap();
    if let Some(old) = props.insert(id, dp){
      if old.typ == TYPE_OBJECT {
        let _o = DataObject {
          byte_ref: old.byte_ref,
        };
      }
      else if old.typ == TYPE_LIST {
        let _o = DataArray {
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
    let handle = BytesRef::get(o.byte_ref, 0, 24);
    self.set_property(key, TYPE_OBJECT, handle);
  }
  
  pub fn put_list(&mut self, key:&str, a:DataArray) {
    let handle = BytesRef::get(a.byte_ref, 0, 24);
    self.set_property(key, TYPE_LIST, handle);
  }
  
  pub fn put_null(&mut self, key:&str) {
    let ba = BytesRef::push(Vec::<u8>::new());
    self.set_property(key, TYPE_NULL, ba);
  }
}

impl<'a> IntoIterator for &'a DataObject {
    type Item = DataProperty;
    type IntoIter = DataObjectIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        DataObjectIterator {
            data_object: self,
            index: 0,
        }
    }
}

pub struct DataObjectIterator<'a> {
    data_object: &'a DataObject,
    index: usize,
}

impl<'a> Iterator for DataObjectIterator<'a> {
    type Item = DataProperty;
    fn next(&mut self) -> Option<DataProperty> {
        let mut handle = BytesRef::get(self.data_object.byte_ref, 0, 24);
        let bytes = handle.from_handle();
        let vec = bytes.as_propertyvec();
        if self.index == vec.len() { return None; }
        let val = vec[self.index];
        self.index += 1;
        Some(val)
    }
}

impl Drop for DataObject {
  fn drop(&mut self) {
    let mut handle = BytesRef::get(self.byte_ref, 0, 24);
    let n = handle.count();
    let bytes = handle.from_handle();
    let mut objects_to_kill = Vec::<DataObject>::new();
    let mut arrays_to_kill = Vec::<DataArray>::new();
    if n == 2 {
      for (_key, old) in bytes.as_propertymap().iter() {
        let ba = BytesRef::get(old.byte_ref, old.off, old.len);
        if old.typ == TYPE_OBJECT { objects_to_kill.push(DataObject { byte_ref: ba.byte_ref, }); }
        else if old.typ == TYPE_LIST { arrays_to_kill.push(DataArray { byte_ref: ba.byte_ref, }); }
        else { ba.decr(); }
      }
    }
    handle.decr();
    bytes.decr();
  }
}

impl fmt::Debug for DataObject {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let val = self.to_json();
    write!(f, "{}", val)
  }
}

