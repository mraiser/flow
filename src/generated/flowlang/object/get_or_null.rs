use ndata::dataobject::*;
use ndata::data::Data;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_property("a");
let a1 = o.get_property("b");
let ax = get_or_null(a0, a1);
let mut o = DataObject::new();
o.set_property("a", ax);
o
}

pub fn get_or_null(a:Data, b:Data) -> Data {
if a.is_object(){
  let a = a.object();
  let b = b.string();
  if a.has(&b) {
    return a.get_property(&b);
  }
  return Data::DNull;
}
else if a.is_array() {
  let a = a.array();
  let b = b.int() as usize;
  if b < a.len() {
    return a.get_property(b);
  }
  return Data::DNull;
}
panic!("The get operation is not supported for this type ({:?})", a);
}

