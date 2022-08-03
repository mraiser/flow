use ndata::dataobject::*;
use ndata::data::Data;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_property("a");
let a1 = o.get_property("b");
let ax = remove(a0, a1);
let mut o = DataObject::new();
o.set_property("a", ax);
o
}

pub fn remove(a:Data, b:Data) -> Data {
if a.is_array() {
  let mut aa = a.array();
  let b = b.int() as usize;
  aa.remove_property(b)
}
else {
  let mut aa = a.object();
  let b = b.string();
  aa.remove_property(&b);
}
a
}

