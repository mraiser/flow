use std::sync::RwLock;
//use state::Storage;
use std::sync::Once;
use std::collections::HashMap;

use ndata::dataobject::*;
use ndata::dataarray::*;
//use ndata::sharedmutex::SharedMutex;

use crate::flowlang::*;
use crate::rustcmd::*;

static START: Once = Once::new();
//pub static PRIMITIVES:Storage<RwLock<HashMap<String, (Transform, String)>>> = Storage::new();
static mut PRIMITIVES:RwLock<Option<HashMap<String, (Transform, String)>>> = RwLock::new(None);

pub struct Primitive {
  pub name: String,
  pub func: Transform,
  pub io: String,
}

impl Primitive {
  #[allow(static_mut_refs)]
  fn init(){
    START.call_once(|| {
      let mut map = HashMap::<String, (Transform, String)>::new();
      map.insert("+".to_string(), (math::plus::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("-".to_string(), (math::minus::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("*".to_string(), (math::multiply::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("/".to_string(), (math::divide::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("<".to_string(), (math::less_than::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert(">".to_string(), (math::greater_than::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("or".to_string(), (math::or::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("time".to_string(), (system::time::execute, "{ in: {}, out: { a: {} } }".to_string()));
      map.insert("unique_session_id".to_string(), (system::unique_session_id::execute, "{ in: {}, out: { a: {} } }".to_string()));
      map.insert("sleep".to_string(), (system::sleep::execute, "{ in: { millis: {} }, out: {} }".to_string()));
      map.insert("stdout".to_string(), (system::stdout::execute, "{ in: { a: {} }, out: {} }".to_string()));
      map.insert("system_call".to_string(), (system::system_call::execute, "{ in: { command: {} }, out: {} }".to_string()));
      map.insert("split".to_string(), (string::split::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("trim".to_string(), (string::trim::execute, "{ in: { a: {} }, out: { a: {} } }".to_string()));
      map.insert("ends_with".to_string(), (string::ends_with::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("starts_with".to_string(), (string::starts_with::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("string_left".to_string(), (string::left::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("string_right".to_string(), (string::right::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("substring".to_string(), (string::substring::execute, "{ in: { a: {}, b: {}, c: {} }, out: { a: {} } }".to_string()));
      map.insert("get".to_string(), (object::get::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("get_or_null".to_string(), (object::get_or_null::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("set".to_string(), (object::set::execute, "{ in: { object: {}, key: {}, value: {} }, out: { a: {} } }".to_string()));
      map.insert("remove".to_string(), (object::remove::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("equals".to_string(), (object::equals::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("length".to_string(), (string::length::execute, "{ in: { a: {} }, out: { a: {} } }".to_string()));
      map.insert("execute_command".to_string(), (system::execute_command::execute, "{ in: { lib: {}, ctl: {}, cmd: {}, params: {}}, out: { a: {} } }".to_string()));
      map.insert("thread".to_string(), (system::thread::execute, "{ in: { lib: {}, ctl: {}, cmd: {}, params: {}}, out: {} }".to_string()));
      map.insert("execute_id".to_string(), (system::execute_id::execute, "{ in: { lib: {}, id: {}, params: {}}, out: { a: {} } }".to_string()));
      map.insert("thread_id".to_string(), (system::thread_id::execute, "{ in: { lib: {}, id: {}, params: {}}, out: {} }".to_string()));
      map.insert("to_json".to_string(), (object::to_json::execute, "{ in: { a: {} }, out: { a: {} } }".to_string()));
      map.insert("object_from_json".to_string(), (object::object_from_json::execute, "{ in: { a: {} }, out: { a: {} } }".to_string()));
      map.insert("array_from_json".to_string(), (object::array_from_json::execute, "{ in: { a: {} }, out: { a: {} } }".to_string()));
      map.insert("has".to_string(), (object::has::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("keys".to_string(), (object::keys::execute, "{ in: { a: {} }, out: { a: {} } }".to_string()));
      map.insert("index_of".to_string(), (object::index_of::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("push".to_string(), (object::push::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("push_all".to_string(), (object::push_all::execute, "{ in: { a: {}, b: {} }, out: { a: {} } }".to_string()));
      map.insert("tcp_listen".to_string(), (tcp::listen::execute, "{ in: { address: {}, port: {} }, out: { a: {} } }".to_string()));
      map.insert("tcp_accept".to_string(), (tcp::accept::execute, "{ in: { listener: {} }, out: { a: {} } }".to_string()));
      map.insert("file_read_all_string".to_string(), (file::read_all_string::execute, "{ in: { path: {} }, out: { a: {} } }".to_string()));
      map.insert("file_read_properties".to_string(), (file::read_properties::execute, "{ in: { path: {} }, out: { a: {} } }".to_string()));
      map.insert("file_write_properties".to_string(), (file::write_properties::execute, "{ in: { path: {}, data: {} }, out: { a: {} } }".to_string()));
      map.insert("file_exists".to_string(), (file::exists::execute, "{ in: { path: {} }, out: { a: {} } }".to_string()));
      map.insert("file_visit".to_string(), (file::visit::execute, "{ in: { path: {}, recursive: {}, lib: {}, ctl: {}, cmd: {} }, out: { a: {} } }".to_string()));
      map.insert("file_is_dir".to_string(), (file::is_dir::execute, "{ in: { path: {} }, out: { a: {} } }".to_string()));
      map.insert("mime_type".to_string(), (file::mime_type::execute, "{ in: { path: {} }, out: { a: {} } }".to_string()));
      map.insert("file_list".to_string(), (file::list::execute, "{ in: { path: {} }, out: { a: {} } }".to_string()));
      map.insert("data_read".to_string(), (data::read::execute, "{ in: { lib: {}, id: {} }, out: { a: {} } }".to_string()));
      map.insert("data_write".to_string(), (data::write::execute, "{ in: { lib: {}, id: {}, data: {}, readers: {}, writers: {} }, out: {} }".to_string()));
      map.insert("data_exists".to_string(), (data::exists::execute, "{ in: { lib: {}, id: {} }, out: { a: {} } }".to_string()));
      map.insert("library_exists".to_string(), (data::library_exists::execute, "{ in: { lib: {} }, out: { a: {} } }".to_string()));
      map.insert("library_new".to_string(), (data::library_new::execute, "{ in: { lib: {}, readers: {}, writers: {} }, out: { a: {} } }".to_string()));
      map.insert("data_root".to_string(), (data::root::execute, "{ in: {}, out: { a: {} } }".to_string()));
      map.insert("http_listen".to_string(), (http::listen::execute, "{ in: { socket_address: {}, library: {}, control: {}, command: {} }, out: { a: {} } }".to_string()));
      map.insert("cast_params".to_string(), (http::cast_params::execute, "{ in: { lib: {}, ctl: {}, cmd: {}, params: {} }, out: { a: {} } }".to_string()));
      map.insert("http_hex_decode".to_string(), (http::hex_decode::execute, "{ in: { input: {} }, out: { a: {} } }".to_string()));
      map.insert("http_hex_encode".to_string(), (http::hex_encode::execute, "{ in: { input: {} }, out: { a: {} } }".to_string()));
      map.insert("http_websocket_open".to_string(), (http::websocket::execute, "{ in: { stream_id: {}, key: {} }, out: { a: {} } }".to_string()));
      map.insert("http_websocket_read".to_string(), (http::websocket_read::execute, "{ in: { stream_id: {} }, out: { a: {} } }".to_string()));
      map.insert("http_websocket_write".to_string(), (http::websocket_write::execute, "{ in: { stream_id: {}, msg: {} }, out: { a: {} } }".to_string()));
      map.insert("to_int".to_string(), (types::to_int::execute, "{ in: { a: {} }, out: { a: {} } }".to_string()));
      map.insert("to_float".to_string(), (types::to_float::execute, "{ in: { a: {} }, out: { a: {} } }".to_string()));
      map.insert("to_boolean".to_string(), (types::to_boolean::execute, "{ in: { a: {} }, out: { a: {} } }".to_string()));
      map.insert("to_string".to_string(), (types::to_string::execute, "{ in: { a: {} }, out: { a: {} } }".to_string()));
      map.insert("is_string".to_string(), (types::is_string::execute, "{ in: { a: {} }, out: { a: {} } }".to_string()));
      map.insert("is_object".to_string(), (types::is_object::execute, "{ in: { a: {} }, out: { a: {} } }".to_string()));
      unsafe { *PRIMITIVES.write().unwrap() = Some(map); }
    });
  }
  
  #[allow(static_mut_refs)]
  pub fn new(name: &str) -> Primitive {
    Primitive::init();
    unsafe {
      let map = &mut PRIMITIVES.read().unwrap();
      let map = map.as_ref().unwrap();
      let t = map.get(name);
      if t.is_none() { panic!("No such primitive {}", name); }
      let t = t.unwrap();
      Primitive { 
        name: name.to_string(),
        func: t.0,
        io: t.1.to_owned(),
      }
    }
  }
  
  pub fn execute(&self, args:DataObject) -> DataObject {
    (self.func)(args)
  }
  
  #[allow(static_mut_refs)]
  pub fn list() -> DataArray {
    Primitive::init();
    unsafe {
      let map = &mut PRIMITIVES.read().unwrap();
      let map = map.as_ref().unwrap();
      let mut array = DataArray::new();
      for key in map.keys() {
        let value = map.get(key).unwrap();
        let mut d = DataObject::new();
        d.put_string("name", key);
        d.put_string("io", &value.1);
        array.push_object(d);
      }
      array
    }
  }
}

