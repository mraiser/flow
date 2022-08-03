use std::sync::RwLock;
use state::Storage;
use std::sync::Once;
use jni::*;
use jni::objects::JValue;
use std::sync::Arc;

use ndata::dataobject::*;

use crate::code::*;
use crate::datastore::*;

static START: Once = Once::new();
static EXECUTOR:Storage<RwLock<Executor>> = Storage::new();

#[derive(Debug)]
pub struct JavaCmd {
  lib:String,
  id:String,
}

impl JavaCmd{
  pub fn init(){
    START.call_once(|| {
      let store = DataStore::new();
      let toproot = store.root;
      let toproot = toproot.canonicalize().unwrap();
      let toproot = toproot.parent().unwrap();
      let binroot = toproot.join("bin");
      let jvm_args = InitArgsBuilder::new()
          .version(JNIVersion::V8)
          .option("-Xcheck:jni")
          .option(&format!("-Djava.class.path={}", binroot.to_str().unwrap()))
          .build()
          .unwrap();
      
//      let storeroot = toproot.join("runtime");
//      let storeroot = storeroot.join("botmanager");
      let jvm = JavaVM::new(jvm_args).unwrap();
      let exec = Executor::new(Arc::new(jvm));
      let _val = exec.with_attached(|env| {
        let cls = env.find_class("Startup").expect("missing class");
        let ss = env.new_string(toproot.to_str().unwrap()).unwrap();
        let s = JValue::Object(ss.into());
        let r = env.call_static_method(cls, "initFromRust", "(Ljava/lang/String;)V", &[s]);
        if r.is_err() { println!("Error initializing JavaCmd: {:?}", r); }
        Ok("OK")
      });
      EXECUTOR.set(RwLock::new(exec));
    });
  }
  
  pub fn new(lib:&str, id:&str) -> JavaCmd{
    JavaCmd{
      lib:lib.to_string(),
      id:id.to_string(),
    }
  }
  
  pub fn execute(&self, args:DataObject) -> Result<DataObject, CodeException> {
    JavaCmd::init();
    let exec = &mut EXECUTOR.get().write().unwrap();
    let val = exec.with_attached(|env| {
      let cls = env.find_class("Startup").expect("missing class");
      let ss = env.new_string(self.lib.to_owned()).unwrap();
      let s1 = JValue::Object(ss.into());
      let ss = env.new_string(self.id.to_owned()).unwrap();
      let s2 = JValue::Object(ss.into());
      let ss = env.new_string(&args.to_json().to_string()).unwrap();
      let s3 = JValue::Object(ss.into());
      let r = env.call_static_method(cls, "executeFromRust", "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;", &[s1, s2, s3]);
      let sss:String = env.get_string(r.unwrap().l().unwrap().into())?.into();
      Ok(sss)
    });
    Ok(DataObject::from_json(serde_json::from_str(&val.unwrap()).unwrap()))
  }
}

