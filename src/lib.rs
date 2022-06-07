use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::jstring;
use serde_json::*;
use std::sync::Once;
use std::env;
use ndata::dataobject::*;

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
    let output:JString;
    {
      let name: String = env.get_string(name).expect("Couldn't get java string!").into();
      let args: String = env.get_string(args).expect("Couldn't get java string!").into();
      let args = serde_json::from_str(&args).unwrap();
      let args = DataObject::from_json(args);
      
      let prim = Primitive::new(&name);
      let result = prim.execute(args);
      
      output = env.new_string(&result.to_json().to_string()).expect("Couldn't create java string!");
    }
    DataStore::gc();
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
