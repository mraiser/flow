use ndata::dataobject::*;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("a");
let a1 = o.get_int("b");
let ax = right(a0, a1);
let mut o = DataObject::new();
o.put_string("a", &ax);
o
}

pub fn right(a:String, b:i64) -> String {
let b = b as usize;
let b = a.len() - b;
a[b..].to_string()
}

