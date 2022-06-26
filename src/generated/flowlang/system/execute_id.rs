use ndata::dataobject::*;
use ndata::data::*;
use crate::command::*;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("lib");
let a1 = o.get_string("id");
let a2 = o.get_object("params");
let ax = execute_id(a0, a1, a2);
let mut o = DataObject::new();
o.put_object("a", ax);
o
}

pub fn execute_id(mut lib:String, mut id:String, mut params:DataObject) -> DataObject {
let cmd = Command::new(&lib, &id);
cmd.execute(params).unwrap()
}

