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
      map.insert(">".to_string(), (flowlang::math::greater_than::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("or".to_string(), (flowlang::math::or::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("time".to_string(), (flowlang::system::time::execute, "{ in: {}, out: { a: {} } }".to_string()));
      map.insert("unique_session_id".to_string(), (flowlang::system::unique_session_id::execute, "{ in: {}, out: { a: {} } }".to_string()));
      map.insert("sleep".to_string(), (flowlang::system::sleep::execute, "{ in: { millis: {} }, out: {} }".to_string()));
      map.insert("stdout".to_string(), (flowlang::system::stdout::execute, "{ in: { a: {} }, out: {} }".to_string()));
      map.insert("system_call".to_string(), (flowlang::system::system_call::execute, "{ in: { command: {} }, out: {} }".to_string()));
      map.insert("split".to_string(), (flowlang::string::split::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("trim".to_string(), (flowlang::string::trim::execute, "{ in: { a: {} }, out: { a: {} } }".to_string()));
      map.insert("ends_with".to_string(), (flowlang::string::ends_with::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("starts_with".to_string(), (flowlang::string::starts_with::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("string_left".to_string(), (flowlang::string::left::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("string_right".to_string(), (flowlang::string::right::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("substring".to_string(), (flowlang::string::substring::execute, "{ in: { a: {}, b: {}, c: {} }, out: { a: {} } }".to_string()));
      map.insert("get".to_string(), (flowlang::object::get::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("get_or_null".to_string(), (flowlang::object::get_or_null::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("set".to_string(), (flowlang::object::set::execute, "{ in: { object: {}, key: {}, value: {} }, out: { a: {} } }".to_string()));
      map.insert("remove".to_string(), (flowlang::object::remove::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("equals".to_string(), (flowlang::object::equals::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("length".to_string(), (flowlang::string::length::execute, "{ in: { a: {} }, out: { a: {} } }".to_string()));
      map.insert("execute_command".to_string(), (flowlang::system::execute_command::execute, "{ in: { lib: {}, ctl: {}, cmd: {}, params: {}}, out: { a: {} } }".to_string()));
      map.insert("thread".to_string(), (flowlang::system::thread::execute, "{ in: { lib: {}, ctl: {}, cmd: {}, params: {}}, out: {} }".to_string()));
      map.insert("execute_id".to_string(), (flowlang::system::execute_id::execute, "{ in: { lib: {}, id: {}, params: {}}, out: { a: {} } }".to_string()));
      map.insert("thread_id".to_string(), (flowlang::system::thread_id::execute, "{ in: { lib: {}, id: {}, params: {}}, out: {} }".to_string()));
      map.insert("to_json".to_string(), (flowlang::object::to_json::execute, "{ in: { a: {} }, out: { a: {} } }".to_string()));
      map.insert("object_from_json".to_string(), (flowlang::object::object_from_json::execute, "{ in: { a: {} }, out: { a: {} } }".to_string()));
      map.insert("array_from_json".to_string(), (flowlang::object::array_from_json::execute, "{ in: { a: {} }, out: { a: {} } }".to_string()));
      map.insert("has".to_string(), (flowlang::object::has::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("keys".to_string(), (flowlang::object::keys::execute, "{ in: { a: {} }, out: { a: {} } }".to_string()));
      map.insert("index_of".to_string(), (flowlang::object::index_of::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("push".to_string(), (flowlang::object::push::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("push_all".to_string(), (flowlang::object::push_all::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("tcp_listen".to_string(), (flowlang::tcp::listen::execute, "{ in: { address: {}, port: {} }, out: { a: {} } }".to_string()));
      map.insert("tcp_accept".to_string(), (flowlang::tcp::accept::execute, "{ in: { listener: {} }, out: { a: {} } }".to_string()));
      map.insert("file_read_all_string".to_string(), (flowlang::file::read_all_string::execute, "{ in: { path: {} }, out: { a: {} } }".to_string()));
      map.insert("file_read_properties".to_string(), (flowlang::file::read_properties::execute, "{ in: { path: {} }, out: { a: {} } }".to_string()));
      map.insert("file_exists".to_string(), (flowlang::file::exists::execute, "{ in: { path: {} }, out: { a: {} } }".to_string()));
      map.insert("file_visit".to_string(), (flowlang::file::visit::execute, "{ in: { path: {}, recursive: {}, lib: {}, ctl: {}, cmd: {} }, out: { a: {} } }".to_string()));
      map.insert("file_is_dir".to_string(), (flowlang::file::is_dir::execute, "{ in: { path: {} }, out: { a: {} } }".to_string()));
      map.insert("mime_type".to_string(), (flowlang::file::mime_type::execute, "{ in: { path: {} }, out: { a: {} } }".to_string()));
      map.insert("file_list".to_string(), (flowlang::file::list::execute, "{ in: { path: {} }, out: { a: {} } }".to_string()));
      map.insert("data_read".to_string(), (flowlang::data::read::execute, "{ in: { lib: {}, id: {} }, out: { a: {} } }".to_string()));
      map.insert("data_write".to_string(), (flowlang::data::write::execute, "{ in: { lib: {}, id: {}, data: {}, readers: {}, writers: {} }, out: {} }".to_string()));
      map.insert("data_exists".to_string(), (flowlang::data::exists::execute, "{ in: { lib: {}, id: {} }, out: { a: {} } }".to_string()));
      map.insert("library_exists".to_string(), (flowlang::data::library_exists::execute, "{ in: { lib: {} }, out: { a: {} } }".to_string()));
      map.insert("library_new".to_string(), (flowlang::data::library_new::execute, "{ in: { lib: {}, readers: {}, writers: {} }, out: { a: {} } }".to_string()));
      map.insert("http_listen".to_string(), (flowlang::http::listen::execute, "{ in: { socket_address: {}, library: {}, control: {}, command: {} }, out: { a: {} } }".to_string()));
      map.insert("cast_params".to_string(), (flowlang::http::cast_params::execute, "{ in: { lib: {}, ctl: {}, cmd: {}, params: {} }, out: { a: {} } }".to_string()));
      map.insert("http_hex_decode".to_string(), (flowlang::http::hex_decode::execute, "{ in: { input: {} }, out: { a: {} } }".to_string()));
      map.insert("to_int".to_string(), (flowlang::types::to_int::execute, "{ in: { a: {} }, out: { a: {} } }".to_string()));
      map.insert("to_float".to_string(), (flowlang::types::to_float::execute, "{ in: { a: {} }, out: { a: {} } }".to_string()));
      map.insert("to_boolean".to_string(), (flowlang::types::to_boolean::execute, "{ in: { a: {} }, out: { a: {} } }".to_string()));
      map.insert("to_string".to_string(), (flowlang::types::to_string::execute, "{ in: { a: {} }, out: { a: {} } }".to_string()));
      map.insert("is_string".to_string(), (flowlang::types::is_string::execute, "{ in: { a: {} }, out: { a: {} } }".to_string()));
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

