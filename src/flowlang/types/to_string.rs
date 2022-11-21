use ndata::dataobject::*;
use ndata::data::*;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_property("a");
let ax = to_string(a0);
let mut o = DataObject::new();
o.put_string("a", &ax);
o
}

pub fn to_string(a:Data) -> String {
Data::as_string(a)
}

