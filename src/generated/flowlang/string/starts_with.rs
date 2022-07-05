use ndata::dataobject::*;
use ndata::data::*;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("a");
let a1 = o.get_string("b");
let ax = starts_with(a0, a1);
let mut o = DataObject::new();
o.put_bool("a", ax);
o
}

pub fn starts_with(mut a:String, mut b:String) -> bool {
a.starts_with(&b)
}

