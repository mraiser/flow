use ndata::dataobject::*;
use ndata::data::*;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("a");
let a1 = o.get_i64("b");
let a2 = o.get_i64("c");
let ax = substring(a0, a1, a2);
let mut o = DataObject::new();
o.put_str("a", &ax);
o
}

pub fn substring(mut a:String, mut b:i64, mut c:i64) -> String {
let b = b as usize;
let c = c as usize;
a[b..c].to_string()
}

