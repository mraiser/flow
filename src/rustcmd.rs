use std::sync::RwLock;
use state::Storage;
use std::sync::Once;
use std::collections::HashMap;

use ndata::dataobject::*;

use crate::code::*;

pub type Transform = fn(DataObject) -> DataObject;

static START: Once = Once::new();
static COMMANDS:Storage<RwLock<HashMap<String, (Transform, String)>>> = Storage::new();

#[derive(Debug)]
pub struct RustCmd {
  func:Transform,
}

impl RustCmd{
  pub fn init(){
    START.call_once(|| {
      let map = HashMap::<String, (Transform, String)>::new();
      COMMANDS.set(RwLock::new(map));
    });
  }
  
  pub fn add(id: String, t: Transform, io: String) {
    let map = &mut COMMANDS.get().write().unwrap();
    map.insert(id, (t, io));
  }
  
  pub fn new(id:&str) -> RustCmd{
    let map = &mut COMMANDS.get().write().unwrap();
    let t = map.get(id);
    if t.is_none() { panic!("No such command {}", id); }
    let t = t.unwrap();
    RustCmd{
      func:t.0,
    }
  }
  
  pub fn execute(&self, args:DataObject) -> Result<DataObject, CodeException> {
    Ok((self.func)(args))
  }
}

