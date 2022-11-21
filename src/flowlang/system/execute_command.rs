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
let ret = cmd.return_type.to_owned();
let o = cmd.execute(params).unwrap();

if ret == "FLAT" { return o; }

let key;
if o.has("data") { key = "data".to_string(); }
else if o.has("msg") { key = "msg".to_string(); }
else if o.has("a") { key = "a".to_string(); }
else {
  let params = o.clone().keys();
  if params.len() == 0 { 
    return o; 
  }
  key = params[0].to_owned();
}
let val = o.get_property(&key);
let mut o = DataObject::new();
if ret == "String" { o.set_property("msg", val); }
else { o.set_property("data", val); }
o.put_string("status", "ok");
o

}

