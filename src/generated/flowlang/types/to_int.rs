use ndata::dataobject::*;
use ndata::data::*;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_property("a");
let ax = to_int(a0);
let mut o = DataObject::new();
o.put_i64("a", ax);
o
}

pub fn to_int(a:Data) -> i64 {
let s = Data::as_string(a);
s.parse::<i64>().unwrap()
}

