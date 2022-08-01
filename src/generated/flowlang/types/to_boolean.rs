use ndata::dataobject::*;
use ndata::data::*;
use ndata::data::*;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_property("a");
let ax = to_boolean(a0);
let mut o = DataObject::new();
o.put_bool("a", ax);
o
}

pub fn to_boolean(mut a:Data) -> bool {
let s = Data::as_string(a);
s.parse::<bool>().unwrap()
}

