use ndata::dataobject::*;
use ndata::data::*;
use std::thread;

use crate::command::Command;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("lib");
let a1 = o.get_string("id");
let a2 = o.get_object("params");
let ax = thread_id(a0, a1, a2);
let mut o = DataObject::new();
o.put_i64("a", ax);
o
}

pub fn thread_id(mut lib:String, mut id:String, mut params:DataObject) -> i64 {
thread::spawn(move || {
  let cmd = Command::new(&lib, &id);
  let _x = cmd.execute(params).unwrap();
});
1
}

