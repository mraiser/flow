use ndata::dataobject::*;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_bool("a");
let a1 = o.get_bool("b");
let ax = or(a0, a1);
let mut o = DataObject::new();
o.put_bool("a", ax);
o
}

pub fn or(a:bool, b:bool) -> bool {
a || b
}

