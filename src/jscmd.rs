use std::sync::RwLock;
use state::Storage;
use std::sync::Once;
use js_sandbox::*;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use serde_json::Value;
use ndata::dataobject::*;
use ndata::dataarray::*;

use crate::code::*;
use crate::datastore::*;

static START: Once = Once::new();
static SCRIPT:Storage<RwLock<WrappedPointer>> = Storage::new();

struct WrappedPointer{
  script: Script,
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
      let store = DataStore::new();
      let toproot = store.root;
      let src = r#"
		    var NNAPI = {};
		    function register(o) { 
		      if (typeof NNAPI[o.lib] == 'undefined') NNAPI[o.lib] = {};
		      if (typeof NNAPI[o.lib][o.ctl] == 'undefined') NNAPI[o.lib][o.ctl] = {};
		      eval(o.js);
		    }
		    function execute(js) 
		    { 
		      return eval(js);
		    }"#;
      let mut script = Script::from_string(src).expect("Initialization succeeds");
      let mut ptr = WrappedPointer{
        script: script,
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
    let returntype = cmd.get_string("returntype");
    
    let cmdname = "NNAPI.".to_string()+(&self.lib)+"."+(&ctl)+"."+&name;
//    println!("{}", cmdname);
    
    // FIXME - Use timestamp instead
    let h1 = calculate_hash(&js);
    
    JSCmd::init();
    let wrap = &mut SCRIPT.get().write().unwrap();
    let mut hasfunc = false;
    let mut h2 = 0;
    {
      let jsfunctions = &mut wrap.map;
      let h3 = jsfunctions.get(&cmdname);
      hasfunc = h3.is_some();
      if hasfunc { h2 = *h3.unwrap(); }
      jsfunctions.insert(cmdname.to_owned(), h1);
    }

    let script = &mut wrap.script;
    
    if !hasfunc || h2 != h1 {
      let mut newjs = cmdname.to_owned()+" = function(";
      let keys = args.duplicate().keys();
      let mut b = false;
      for key in keys {
        if b { newjs += ","; }
        b = true;
        newjs += &key;
      }
      newjs += "){";
      newjs += &js;
      newjs += "};";
//      println!("{}", newjs);
      
      let mut o = DataObject::new();
      o.put_str("lib", &self.lib);
      o.put_str("ctl", &ctl);
      o.put_str("cmd", &name);
      o.put_str("js", &newjs);
//      println!("{}", o.to_json());
      let _: () = script.call("register", &o.to_json()).unwrap();
    }
    
    let var = "x".to_string()+&h1.to_string();
    let mut newjs = "var ".to_string()+&var;
    newjs += " = ";
    newjs += &(args.duplicate().to_json().to_string());
    newjs += "; ";
    newjs += &cmdname;
    newjs += "(";
    
    let keys = args.duplicate().keys();
    let mut b = false;
    for key in keys {
      if b { newjs += ","; }
      b = true;
      newjs += &var;
      newjs += ".";
      newjs += &key;
    }
    newjs += ");";

//    println!("{}", newjs);



    let mut jo = DataObject::new();
    let result: Result<Value, _> = script.call("execute", &newjs);
    if result.is_err() {
      jo.put_str("status", "err");
      let msg = format!("{:?}", result);
      jo.put_str("msg", &msg);
    }
    else {
      jo.put_str("status", "ok");
      let val = result.unwrap();
      if val.is_string() {
        jo.put_str("msg", &val.as_str().unwrap());
      }
      else if val.is_boolean() {
        jo.put_bool("data", val.as_bool().unwrap());
      }
      else if val.is_i64() {
        jo.put_i64("data", val.as_i64().unwrap());
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
