use ndata::dataobject::*;
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

pub fn execute_id(lib:String, id:String, params:DataObject) -> DataObject {
let cmd = Command::new(&lib, &id);
cmd.execute(params).unwrap()
}

