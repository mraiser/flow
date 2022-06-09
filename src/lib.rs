use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::jstring;
use serde_json::*;
use std::sync::Once;
use std::env;
use ndata::dataobject::*;
use std::panic;

mod code;
mod case;
mod command;
mod datastore;
mod primitives;
mod rustcmd;
mod generated;
mod rand;

use crate::primitives::*;
use crate::datastore::*;

static START: Once = Once::new();

#[no_mangle]
pub extern "system" fn Java_com_newbound_code_primitive_NativePrimitiveCall_call(env: JNIEnv,
                                             class: JClass,
                                             name: JString,
                                             args: JString)
                                             -> jstring {
  START.call_once(|| {
    DataStore::init("data");
  });
  
  env::set_var("RUST_BACKTRACE", "1");
  {
    let output:String;
    {
      let hold = DataObject::new();
      let result = panic::catch_unwind(|| {
        let name: String = env.get_string(name).expect("Couldn't get java string!").into();
        let args: String = env.get_string(args).expect("Couldn't get java string!").into();
        let args = serde_json::from_str(&args).unwrap();
        let args = DataObject::from_json(args);
        
        let prim = Primitive::new(&name);
        let result = prim.execute(args);
        
        let output = result.to_json().to_string();
        let mut hold = DataObject::get(hold.data_ref);
        hold.put_str("result", &output);
      });
      
  		match result {
        Ok(_x) => output = hold.get_string("result"),
        Err(e) => {
          
          let s = match e.downcast::<String>() {
            Ok(panic_msg) => format!("{}", panic_msg),
            Err(_) => "unknown error".to_string()
          };        
          output = s;
        }
      }
    }
    DataStore::gc();
    let output = env.new_string(output).expect("Couldn't create java string!");
    return output.into_inner();
  }
}

#[no_mangle]
pub extern "system" fn Java_com_newbound_code_primitive_NativePrimitiveCall_list(env: JNIEnv,
                                             class: JClass)
                                             -> jstring {
  START.call_once(|| {
    DataStore::init("data");
  });
  let output:JString;
  {
    let result = Primitive::list();
    output = env.new_string(&result.to_json().to_string()).expect("Couldn't create java string!");
  }
  DataStore::gc();
  return output.into_inner();
}
