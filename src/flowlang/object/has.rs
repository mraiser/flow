use ndata::dataobject::*;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_object("a");
let a1 = o.get_string("b");
let ax = has(a0, a1);
let mut o = DataObject::new();
o.put_bool("a", ax);
o
}

pub fn has(a:DataObject, b:String) -> bool {
a.has(&b)
}

