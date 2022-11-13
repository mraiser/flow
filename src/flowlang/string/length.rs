use ndata::dataobject::*;
use ndata::data::Data;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_property("a");
let ax = length(a0);
let mut o = DataObject::new();
o.put_i64("a", ax);
o
}

pub fn length(a:Data) -> i64 {
if a.is_string() { return a.string().len() as i64; }
else {
  return a.array().len() as i64
}
}

