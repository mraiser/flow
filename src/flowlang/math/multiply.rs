use ndata::dataobject::*;
use ndata::data::Data;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_property("a");
let a1 = o.get_property("b");
let ax = multiply(a0, a1);
let mut o = DataObject::new();
o.set_property("a", ax);
o
}

pub fn multiply(a:Data, b:Data) -> Data {
if a.is_number() && b.is_number() {
  if a.is_float() || b.is_float() { 
    let c = a.float() * b.float();
    return Data::DFloat(c); 
  }
  else {
    let c = a.int() * b.int();
    return Data::DInt(c);
  }
}  
else {
  return Data::DString("NaN".to_owned());
}

}

