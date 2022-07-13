use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::jstring;
use std::sync::Once;
use std::env;
use ndata::dataobject::*;
use std::panic;

use ::flowlang::command::*;
use ::flowlang::datastore::*;
use ::flowlang::primitives::*;
use ::flowlang::buildrust::*;
use ::flowlang::datastore::*;
use ::flowlang::generated::*;

static START: Once = Once::new();

#[no_mangle]
pub extern "system" fn Java_com_newbound_code_LibFlow_call(env: JNIEnv,
                                             class: JClass,
                                             lib: JString,
                                             ctl: JString,
                                             cmd: JString,
                                             args: JString)
                                             -> jstring {
  START.call_once(|| {
    DataStore::init("data");
    Generated::init();
  });
  
  env::set_var("RUST_BACKTRACE", "1");
  {
    let output:String;
    {
      let hold = DataObject::new();
      let result = panic::catch_unwind(|| {
        let lib: String = env.get_string(lib).expect("Couldn't get java string!").into();
        let ctl: String = env.get_string(ctl).expect("Couldn't get java string!").into();
        let cmd: String = env.get_string(cmd).expect("Couldn't get java string!").into();
        let args: String = env.get_string(args).expect("Couldn't get java string!").into();
        let args = serde_json::from_str(&args).unwrap();
        let args = DataObject::from_json(args);
        
        let cmd = Command::lookup(&lib, &ctl, &cmd);
        let result = cmd.execute(args).unwrap();
        
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
pub extern "system" fn Java_com_newbound_code_LibFlow_build(env: JNIEnv,
                                             class: JClass,
                                             lib: JString,
                                             ctl: JString,
                                             cmd: JString)
                                             -> jstring {
  START.call_once(|| {
    DataStore::init("data");
    Generated::init();
  });
  
  env::set_var("RUST_BACKTRACE", "1");
  {
    let output:String;
    {
      let hold = DataObject::new();
      let result = panic::catch_unwind(|| {
        let lib: String = env.get_string(lib).expect("Couldn't get java string!").into();
        let ctl: String = env.get_string(ctl).expect("Couldn't get java string!").into();
        let cmd: String = env.get_string(cmd).expect("Couldn't get java string!").into();
        build(&lib, &ctl, &cmd);
        let output = "OK".to_string();
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
pub extern "system" fn Java_com_newbound_code_LibFlow_list(env: JNIEnv,
                                             class: JClass)
                                             -> jstring {
  START.call_once(|| {
    DataStore::init("data");
    Generated::init();
  });
  let output:JString;
  {
    let result = Primitive::list();
    output = env.new_string(&result.to_json().to_string()).expect("Couldn't create java string!");
  }
  DataStore::gc();
  return output.into_inner();
}

