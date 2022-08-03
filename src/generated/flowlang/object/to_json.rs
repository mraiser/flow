use ndata::dataobject::*;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_object("a");
let ax = to_json(a0);
let mut o = DataObject::new();
o.put_str("a", &ax);
o
}

pub fn to_json(a:DataObject) -> String {
a.to_json().to_string()
}

