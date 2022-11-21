use ndata::dataobject::*;
use ndata::dataarray::*;
use std::fs;

use crate::flowlang::system::execute_command::*;
pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("path");
let a1 = o.get_boolean("recursive");
let a2 = o.get_string("lib");
let a3 = o.get_string("ctl");
let a4 = o.get_string("cmd");
let ax = visit(a0, a1, a2, a3, a4);
let mut o = DataObject::new();
o.put_array("a", ax);
o
}

pub fn visit(path:String, recursive:bool, lib:String, ctl:String, cmd:String) -> DataArray {
let mut a = DataArray::new();

for file in fs::read_dir(&path).unwrap() {
  let path = file.unwrap().path();
  let name = &path.display().to_string();
  let mut args = DataObject::new();
  args.put_string("path", &name);
  let o = execute_command(lib.to_owned(), ctl.to_owned(), cmd.to_owned(), args);
  if o.has("a") {
    a.push_property(o.get_property("a"));
  }
  
  if recursive && path.is_dir() {
    let a2 = visit(name.to_string(), recursive, lib.to_owned(), ctl.to_owned(), cmd.to_owned());
    a.join(a2);
  }
}

a
}

