use ndata::dataobject::*;
use ndata::data::Data;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_property("a");
let a1 = o.get_property("b");
let ax = index_of(a0, a1);
let mut o = DataObject::new();
o.put_i64("a", ax);
o
}

pub fn index_of(a:Data, b:Data) -> i64 {
if a.is_array() {
  let a = a.array();
  let mut i = 0;
  let n = a.len();
  while i<n {
    let d = a.get_property(i);
    if Data::equals(d,b.clone()) { return i as i64; }
    i = i + 1;
  }
}
else {
  let a = a.string();
  let i = a.find(&b.string());
  if i.is_some() { return i.unwrap() as i64; }
}
-1

}

