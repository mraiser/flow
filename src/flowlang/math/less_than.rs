use ndata::dataobject::*;
use ndata::data::Data;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_property("a");
let a1 = o.get_property("b");
let ax = less_than(a0, a1);
let mut o = DataObject::new();
o.put_boolean("a", ax);
o
}

pub fn less_than(a:Data, b:Data) -> bool {
if a.is_number() && b.is_number() {
  if a.is_float() {
    if b.is_float() {
      return a.float() < b.float();
    }
    return a.float() < (b.int() as f64);
  }
  if b.is_int() {
    return a.int() < b.int();
  }
  return (a.int() as f64) < b.float();
}
a.string().cmp(&b.string()).is_lt()
}

