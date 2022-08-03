use ndata::dataobject::*;
use crate::command::*;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("lib");
let a1 = o.get_string("ctl");
let a2 = o.get_string("cmd");
let a3 = o.get_object("params");
let ax = execute_command(a0, a1, a2, a3);
let mut o = DataObject::new();
o.put_object("a", ax);
o
}

pub fn execute_command(lib:String, ctl:String, cmd:String, params:DataObject) -> DataObject {
let cmd = Command::lookup(&lib, &ctl, &cmd);
cmd.execute(params).unwrap()
}

