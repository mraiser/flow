use ndata::dataobject::*;
use ndata::data::Data;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_property("a");
let ax = is_object(a0);
let mut o = DataObject::new();
o.put_boolean("a", ax);
o
}

pub fn is_object(a:Data) -> bool {
a.is_object()
}

