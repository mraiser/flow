use std::sync::RwLock;
//use state::Storage;
//use ndata::sharedmutex::SharedMutex;
use std::sync::Once;
use std::collections::HashMap;

use ndata::dataobject::*;

use crate::code::*;

pub type Transform = fn(DataObject) -> DataObject;

static START: Once = Once::new();
//static COMMANDS:Storage<RwLock<HashMap<String, (Transform, String)>>> = Storage::new();
static mut COMMANDS:RwLock<Option<HashMap<String, (Transform, String)>>> = RwLock::new(None);

#[derive(Debug)]
pub struct RustCmd {
  func:Transform,
}

impl RustCmd{
  pub fn init(){
    START.call_once(|| {
      let map = HashMap::<String, (Transform, String)>::new();
      unsafe { *COMMANDS.write().unwrap() = Some(map); }
    });
  }
  
  pub fn add(id: String, t: Transform, io: String) {
    unsafe {
      let map = &mut COMMANDS.write().unwrap();
      let map = map.as_mut().unwrap();
      map.insert(id, (t, io));
    }
  }
  
  pub fn new(id:&str) -> RustCmd{
    unsafe {
      let map = &mut COMMANDS.read().unwrap();
      let map = map.as_ref().unwrap();
      let t = map.get(id);
      if t.is_none() { panic!("No such command {}", id); }
      let t = t.unwrap();
      RustCmd{
        func:t.0,
      }
    }
  }
  
  pub fn exists(id:&str) -> bool{
    unsafe {
      let map = &mut COMMANDS.read().unwrap();
      let map = map.as_ref().unwrap();
      let t = map.get(id);
      if t.is_none() { return false; }
    }
    true
  }
  
  pub fn execute(&self, args:DataObject) -> Result<DataObject, CodeException> {
    Ok((self.func)(args))
  }
}

