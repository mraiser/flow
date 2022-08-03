use ndata::dataobject::*;
use ndata::data::Data;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_property("a");
let a1 = o.get_property("b");
let ax = equals(a0, a1);
let mut o = DataObject::new();
o.put_bool("a", ax);
o
}

pub fn equals(a:Data, b:Data) -> bool {
Data::equals(a, b)
}

