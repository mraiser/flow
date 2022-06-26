use ndata::dataobject::*;
use ndata::data::*;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_property("a");
let a1 = o.get_property("b");
let ax = get(a0, a1);
let mut o = DataObject::new();
o.set_property("a", ax);
o
}

pub fn get(mut a:Data, mut b:Data) -> Data {
if a.is_object(){
  return a.object().get_property(&b.string());
}
else if a.is_array() {
  return a.array().get_property(b.int() as usize);
}
else {
  panic!("The get operation is not supported for this type ({:?})", a);
}
}

