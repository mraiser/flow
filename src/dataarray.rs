use serde_json::*;
use std::fmt;

use crate::bytesref::*;
use crate::bytesutil::*;
use crate::dataproperty::*;
use crate::dataobject::*;

pub struct DataArray {
  pub byte_ref: usize,
}

impl DataArray {
  pub fn from_json(value:Value) -> DataArray {
    let bytes: Vec<u8> = Vec::<u8>::new();
    let ba = BytesRef::push(bytes);
    let ba = ba.to_handle();
    ba.incr();
    let mut o = DataArray {
      byte_ref: ba.byte_ref,
    };
    
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
    for old in self {
      if old.typ == TYPE_LONG { val.push(json!(self.get_i64(id))); }
      else if old.typ == TYPE_FLOAT { val.push(json!(self.get_f64(id))); }
      else if old.typ == TYPE_BOOLEAN { val.push(json!(self.get_bool(id))); }
      else if old.typ == TYPE_STRING { val.push(json!(self.get_string(id))); }
      else if old.typ == TYPE_OBJECT { val.push(self.get_object(id).to_json()); }
      else if old.typ == TYPE_LIST { val.push(self.get_array(id).to_json()); }
      else { val.push(json!(null)); }
      id = id + 1;
    }
    json!(val)
  }
  
  pub fn len(&self) -> usize {
    let mut handle:BytesRef = BytesRef::get(self.byte_ref, 0, 24);
    let bytes = handle.from_handle();
    bytes.len / PROPERTY_SIZE as usize
  }

  pub fn get_property(&self, id:usize) -> DataProperty {
    let mut handle:BytesRef = BytesRef::get(self.byte_ref, 0, 24);
    let bytes = handle.from_handle();
    let props = bytes.as_propertyvec();
    props.get(id).unwrap().clone()
  }
  
  pub fn get_string(&self, id:usize) -> String {
    let dp = self.get_property(id);
    let br = BytesRef::get(dp.byte_ref, dp.off, dp.len);
    br.as_string()
  }
  
  pub fn get_bool(&self, id:usize) -> bool {
    let dp = self.get_property(id);
    let br = BytesRef::get(dp.byte_ref, dp.off, dp.len);
    br.as_bool()
  }
  
  pub fn get_i64(&self, id:usize) -> i64 {
    let dp = self.get_property(id);
    let br = BytesRef::get(dp.byte_ref, dp.off, dp.len);
    br.as_i64()
  }
  
  pub fn get_f64(&self, id:usize) -> f64 {
    let dp = self.get_property(id);
    let br = BytesRef::get(dp.byte_ref, dp.off, dp.len);
    br.as_f64()
  }

  pub fn get_array(&self, id:usize) -> DataArray {
    let dp = self.get_property(id);
    let mut br = BytesRef::get(dp.byte_ref, dp.off, dp.len);
    br.incr();
    br.from_handle().incr();
    DataArray { byte_ref: br.byte_ref, }
  }

  pub fn get_object(&self, id:usize) -> DataObject {
    let dp = self.get_property(id);
    let mut br = BytesRef::get(dp.byte_ref, dp.off, dp.len);
    br.incr();
    br.from_handle().incr();
    DataObject { byte_ref: br.byte_ref, }
  }

  pub fn push_property(&mut self, typ:u8, bytesref:BytesRef) {
    // FIXME - Not thread safe. Call should be synchronized
    bytesref.incr();

    let mut handle:BytesRef = BytesRef::get(self.byte_ref, 0, 24);
    let mut bytes = handle.from_handle();
    let mut props = bytes.as_propertyvec();

    let id = props.len();
    let dp = DataProperty::new(id, typ, bytesref);
    props.push(dp);
    
    let nubytes = propertyvec_to_bytes(props);
    let n = nubytes.len();
    bytes.len = n;
    bytes.swap(nubytes);
    handle.swap(bytes.to_handle_bytes());
  }

  pub fn push_str(&mut self, val:&str) {
    let ba = BytesRef::from_str(val);
    self.push_property(TYPE_STRING, ba);
  }
  
  pub fn push_bool(&mut self, val:bool) {
    let ba = BytesRef::from_bool(val);
    self.push_property(TYPE_BOOLEAN, ba);
  }
  
  pub fn push_i64(&mut self, val:i64) {
    let ba = BytesRef::from_i64(val);
    self.push_property(TYPE_LONG, ba);
  }
  
  pub fn push_float(&mut self, val:f64) {
    let ba = BytesRef::from_f64(val);
    self.push_property(TYPE_FLOAT, ba);
  }

  pub fn push_object(&mut self, o:DataObject) {
    let mut handle = BytesRef::get(o.byte_ref, 0, 24);
    handle.from_handle().incr();
    self.push_property(TYPE_OBJECT, handle);
  }
  
  pub fn push_list(&mut self, a:DataArray) {
    let mut handle = BytesRef::get(a.byte_ref, 0, 24);
    handle.from_handle().incr();
    self.push_property(TYPE_LIST, handle);
  }
  
  // FIXME - add insert/set_...(index, value) function for all types
  
  pub fn remove_property(&mut self, id:usize) {
    // FIXME - Not thread safe. Call should be synchronized
    let mut handle:BytesRef = BytesRef::get(self.byte_ref, 0, 24);
    let mut bytes = handle.from_handle();
    let mut props = bytes.as_propertyvec();
    
    let dp = props.remove(id);
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
    
    let nubytes = propertyvec_to_bytes(props);
    let n = nubytes.len();
    bytes.len = n;
    bytes.swap(nubytes);
    handle.swap(bytes.to_handle_bytes());
  }
}

impl<'a> IntoIterator for &'a DataArray {
    type Item = DataProperty;
    type IntoIter = DataArrayIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        DataArrayIterator {
            data_array: self,
            index: 0,
        }
    }
}

pub struct DataArrayIterator<'a> {
    data_array: &'a DataArray,
    index: usize,
}

impl<'a> Iterator for DataArrayIterator<'a> {
    type Item = DataProperty;
    fn next(&mut self) -> Option<DataProperty> {
        let mut handle = BytesRef::get(self.data_array.byte_ref, 0, 24);
        let bytes = handle.from_handle();
        let vec = bytes.as_propertyvec();
        if self.index == vec.len() { return None; }
        let val = vec[self.index];
        self.index += 1;
        Some(val)
    }
}

impl Drop for DataArray {
  fn drop(&mut self) {
    let mut handle = BytesRef::get(self.byte_ref, 0, 24);
    let n = handle.count();
    let bytes = handle.from_handle();
    let mut objects_to_kill = Vec::<DataObject>::new();
    let mut arrays_to_kill = Vec::<DataArray>::new();
    if n == 2 {
      for old in bytes.as_propertyvec().iter() {
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

impl fmt::Debug for DataArray {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let val = self.to_json();
    write!(f, "{}", val)
  }
}

