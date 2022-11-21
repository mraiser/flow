use ndata::dataobject::*;
use ndata::data::Data;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_property("a");
let ax = is_string(a0);
let mut o = DataObject::new();
o.put_boolean("a", ax);
o
}

pub fn is_string(a:Data) -> bool {
a.is_string()
}

