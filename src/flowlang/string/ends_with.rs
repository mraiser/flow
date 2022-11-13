use ndata::dataobject::*;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("a");
let a1 = o.get_string("b");
let ax = ends_with(a0, a1);
let mut o = DataObject::new();
o.put_bool("a", ax);
o
}

pub fn ends_with(a:String, b:String) -> bool {
a.ends_with(&b)
}

