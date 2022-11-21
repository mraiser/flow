use ndata::dataobject::*;
use ndata::data::Data;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_object("object");
let a1 = o.get_string("key");
let a2 = o.get_property("value");
let ax = set(a0, a1, a2);
let mut o = DataObject::new();
o.put_object("a", ax);
o
}

pub fn set(object:DataObject, key:String, value:Data) -> DataObject {
object.clone().set_property(&key, value);
object
}

