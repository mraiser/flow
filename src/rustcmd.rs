use ndata::dataobject::*;

use crate::code::*;

pub type Transform = fn(DataObject) -> DataObject;

#[derive(Debug)]
pub struct RustCmd {
  func:Transform,
}

impl RustCmd{
  pub fn new(t:Transform) -> RustCmd{
    RustCmd{
      func:t,
    }
  }
  
  pub fn execute(&self, args:DataObject) -> Result<DataObject, CodeException> {
    Ok((self.func)(args))
  }
}

