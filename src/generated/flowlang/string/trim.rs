use ndata::dataobject::*;
use ndata::data::*;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("a");
let ax = trim(a0);
let mut o = DataObject::new();
o.put_str("a", &ax);
o
}

pub fn trim(mut a:String) -> String {
a.trim().to_string()
}

