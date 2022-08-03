use ndata::dataobject::*;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("a");
let a1 = o.get_i64("b");
let ax = left(a0, a1);
let mut o = DataObject::new();
o.put_str("a", &ax);
o
}

pub fn left(a:String, b:i64) -> String {
let b = b as usize;
a[..b].to_string()
}

