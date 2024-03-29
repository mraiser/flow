use std::sync::RwLock;
use state::Storage;
use std::sync::Once;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use ndata::dataobject::*;
use ndata::dataarray::*;
use deno_core::JsRuntime;
use deno_core::RuntimeOptions;

use crate::code::*;
use crate::datastore::*;

static START: Once = Once::new();
static SCRIPT:Storage<RwLock<WrappedPointer>> = Storage::new();

struct WrappedPointer{
  runtime: JsRuntime,
  map: HashMap<String, u64>,
}

unsafe impl Send for WrappedPointer {}
unsafe impl Sync for WrappedPointer {}

#[derive(Debug)]
pub struct JSCmd {
  lib:String,
  id:String,
}

impl JSCmd{
  pub fn init(){
    START.call_once(|| {
//      let store = DataStore::new();
//      let toproot = store.root;
      let src = r#"
		    var NNAPI = {};
		    function register(o) { 
		      if (typeof NNAPI[o.lib] == 'undefined') NNAPI[o.lib] = {};
		      if (typeof NNAPI[o.lib][o.ctl] == 'undefined') NNAPI[o.lib][o.ctl] = {};
		      eval(o.js);
		    }"#;
      let mut runtime = JsRuntime::new(RuntimeOptions {
        ..Default::default()
      });
      let _ = runtime.execute_script("<usage>", src).unwrap();
//      println!("{:?}", x);
      let ptr = WrappedPointer{
        runtime: runtime,
        map: HashMap::new(),
      };
      SCRIPT.set(RwLock::new(ptr));
    });
  }
  
  pub fn new(lib:&str, id:&str) -> JSCmd{
    JSCmd{
      lib:lib.to_string(),
      id:id.to_string(),
    }
  }
  
  pub fn execute(&self, args:DataObject) -> Result<DataObject, CodeException> {
    let store = DataStore::new();
    let cmd = store.get_data(&self.lib, &self.id);
    let cmd = cmd.get_object("data");
    let jsid = cmd.get_string("js");
    let name = cmd.get_string("name");
    let cmd = store.get_data(&self.lib, &jsid);    
    let cmd = cmd.get_object("data");
//    println!("{}", cmd.to_json());
    let js = cmd.get_string("js");
    let ctl = cmd.get_string("ctl");
//    let returntype = cmd.get_string("returntype");
    let params = cmd.get_array("params");
    let mut param_names = DataArray::new();
    for param in params.objects() {
      let param_name = param.object().get_string("name");
      param_names.push_string(&param_name);
    }
//    println!("{:?}", param_names.objects());
    
    let cmdname = "NNAPI.".to_string()+(&self.lib)+"."+(&ctl)+"."+&name;
//    println!("{}", cmdname);
    
    // FIXME - Use timestamp instead
    let h1 = calculate_hash(&js);
    
    JSCmd::init();
    let wrap = &mut SCRIPT.get().write().unwrap();
    let hasfunc;
    let mut h2 = 0;
    {
      let jsfunctions = &mut wrap.map;
      let h3 = jsfunctions.get(&cmdname);
      hasfunc = h3.is_some();
      if hasfunc { h2 = *h3.unwrap(); }
      jsfunctions.insert(cmdname.to_owned(), h1);
    }

    let runtime = &mut wrap.runtime;
    
    if !hasfunc || h2 != h1 {
      let mut newjs = cmdname.to_owned()+" = function(";
      let keys = param_names.objects();
      let mut b = false;
      for key in keys {
        if b { newjs += ","; }
        b = true;
        newjs += &key.string();
      }
      newjs += "){";
      newjs += &js;
      newjs += "};";
//      println!("{}", newjs);
      
      let mut o = DataObject::new();
      o.put_string("lib", &self.lib);
      o.put_string("ctl", &ctl);
      o.put_string("cmd", &name);
      o.put_string("js", &newjs);
//      println!("{}", o.to_json());
      let _ = runtime.execute_script("<usage>", &("register(".to_string()+&o.to_json().to_string()+");")).unwrap();
    }
    
    let var = "x".to_string()+&h1.to_string();
    let mut newjs = "var ".to_string()+&var;
    newjs += " = ";
    newjs += &(args.clone().to_json().to_string());
    newjs += "; ";
    newjs += &cmdname;
    newjs += "(";
    
    let keys = param_names.objects();
    let mut b = false;
    for key in keys {
      if b { newjs += ","; }
      b = true;
      newjs += &var;
      newjs += ".";
      newjs += &key.string();
    }
    newjs += ");";

//    println!("{}", newjs);

    let mut jo = DataObject::new();
    let result = runtime.execute_script("<usage>", &newjs);
        
    if result.is_err() {
      jo.put_string("status", "err");
      let msg = format!("{:?}", result);
      jo.put_string("msg", &msg);
    }
    else {
      let mut scope = runtime.handle_scope();
      let local = deno_core::v8::Local::new(&mut scope, result.unwrap());
      let result = serde_v8::from_v8::<serde_json::Value>(&mut scope, local);
      jo.put_string("status", "ok");
      let val = result.unwrap();
      if val.is_string() {
        jo.put_string("msg", &val.as_str().unwrap());
      }
      else if val.is_boolean() {
        jo.put_boolean("data", val.as_bool().unwrap());
      }
      else if val.is_i64() {
        jo.put_int("data", val.as_i64().unwrap());
      }
      else if val.is_f64() {
        jo.put_float("data", val.as_f64().unwrap());
      }
      else if val.is_object() {
        jo.put_object("data", DataObject::from_json(val));
      }
      else if val.is_array() {
        jo.put_array("data", DataArray::from_json(val));
      }
      else if val.is_null() {
        jo.put_null("data");
      }
    }
            
    Ok(jo)
  }
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
  let mut s = DefaultHasher::new();
  t.hash(&mut s);
  s.finish()
}
