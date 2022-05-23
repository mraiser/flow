use ndata::dataobject::*;

use crate::generated::*;
use crate::rustcmd::*;

pub struct Primitive {
  pub name: String,
  pub func: Transform,
}

impl Primitive {
  pub fn new(name: &str) -> Primitive {
    if name == "+" { return Primitive::build(name, flowlang::math::plus::execute); }
    if name == "-" { return Primitive::build(name, flowlang::math::minus::execute); }
    if name == "<" { return Primitive::build(name, flowlang::math::less_than::execute); }
    if name == "time" { return Primitive::build(name, flowlang::system::time::execute); }
    if name == "split" { return Primitive::build(name, flowlang::string::split::execute); }
    if name == "get" { return Primitive::build(name, flowlang::object::get::execute); }
    if name == "length" { return Primitive::build(name, flowlang::string::length::execute); }
    if name == "execute_command" { return Primitive::build(name, flowlang::system::execute_command::execute); }
    if name == "to_json" { return Primitive::build(name, flowlang::object::to_json::execute); }
        
    panic!("No such primitive {}", name);
  }
  
  pub fn execute(&self, args:DataObject) -> DataObject {
    (self.func)(args)
  }
  
  pub fn build(name:&str, func:Transform) -> Primitive {
    return Primitive { 
      name: name.to_string(),
      func: func,
    };
  }
}

