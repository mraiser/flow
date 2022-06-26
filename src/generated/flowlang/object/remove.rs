use ndata::dataobject::*;
use ndata::data::*;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_object("a");
let a1 = o.get_string("b");
let ax = remove(a0, a1);
let mut o = DataObject::new();
o.put_object("a", ax);
o
}

pub fn remove(mut a:DataObject, mut b:String) -> DataObject {
a.remove_property(&b);
a
}

