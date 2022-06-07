use std::sync::RwLock;
use state::Storage;
use std::sync::Once;
use std::collections::HashMap;

use ndata::dataobject::*;
use ndata::dataarray::*;

use crate::generated::*;
use crate::rustcmd::*;

static START: Once = Once::new();
pub static PRIMITIVES:Storage<RwLock<HashMap<String, (Transform, String)>>> = Storage::new();

pub struct Primitive {
  pub name: String,
  pub func: Transform,
  pub io: String,
}

impl Primitive {
  fn init(){
    START.call_once(|| {
      let mut map = HashMap::<String, (Transform, String)>::new();
      map.insert("+".to_string(), (flowlang::math::plus::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("-".to_string(), (flowlang::math::minus::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("*".to_string(), (flowlang::math::multiply::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("/".to_string(), (flowlang::math::divide::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("<".to_string(), (flowlang::math::less_than::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("time".to_string(), (flowlang::system::time::execute, "{ in: {}, out: { a: {} } }".to_string()));
      map.insert("sleep".to_string(), (flowlang::system::sleep::execute, "{ in: { millis: {} }, out: {} }".to_string()));
      map.insert("split".to_string(), (flowlang::string::split::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("get".to_string(), (flowlang::object::get::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("length".to_string(), (flowlang::string::length::execute, "{ in: { a: {} }, out: { a: {} } }".to_string()));
      map.insert("execute_command".to_string(), (flowlang::system::execute_command::execute, "{ in: { lib: {}, ctl: {}, cmd: {}, params: {}}, out: { a: {} } }".to_string()));
      map.insert("to_json".to_string(), (flowlang::object::to_json::execute, "{ in: { a: {} }, out: { a: {} } }".to_string()));
      map.insert("has".to_string(), (flowlang::object::has::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("tcp_listen".to_string(), (flowlang::tcp::listen::execute, "{ in: { address: {}, port: {} }, out: { a: {} } }".to_string()));
      map.insert("tcp_accept".to_string(), (flowlang::tcp::accept::execute, "{ in: { listener: {} }, out: { a: {} } }".to_string()));
      PRIMITIVES.set(RwLock::new(map));
    });
  }
  
  pub fn new(name: &str) -> Primitive {
    Primitive::init();
    let map = &mut PRIMITIVES.get().write().unwrap();
    let t = map.get(name);
    if t.is_none() { panic!("No such primitive {}", name); }
    let t = t.unwrap();
    Primitive { 
      name: name.to_string(),
      func: t.0,
      io: t.1.to_owned(),
    }
  }
  
  pub fn execute(&self, args:DataObject) -> DataObject {
    (self.func)(args)
  }
  
  pub fn list() -> DataArray {
    Primitive::init();
    let map = &mut PRIMITIVES.get().write().unwrap();
    let mut array = DataArray::new();
    for key in map.keys() {
      let value = map.get(key).unwrap();
      let mut d = DataObject::new();
      d.put_str("name", key);
      d.put_str("io", &value.1);
      array.push_object(d);
    }
    array
  }
}

