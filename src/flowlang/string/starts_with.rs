use ndata::dataobject::*;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("a");
let a1 = o.get_string("b");
let ax = starts_with(a0, a1);
let mut o = DataObject::new();
o.put_boolean("a", ax);
o
}

pub fn starts_with(a:String, b:String) -> bool {
a.starts_with(&b)
}

