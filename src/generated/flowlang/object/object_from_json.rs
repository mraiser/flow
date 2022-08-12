use ndata::dataobject::*;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("a");
let ax = object_from_json(a0);
let mut o = DataObject::new();
o.put_object("a", ax);
o
}

pub fn object_from_json(a:String) -> DataObject {
DataObject::from_string(&a)

}

