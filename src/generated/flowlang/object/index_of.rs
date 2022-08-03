use ndata::dataobject::*;
use ndata::dataarray::DataArray;
use ndata::data::Data;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_array("a");
let a1 = o.get_property("b");
let ax = index_of(a0, a1);
let mut o = DataObject::new();
o.put_i64("a", ax);
o
}

pub fn index_of(a:DataArray, b:Data) -> i64 {
let mut i = 0;
let n = a.len();
while i<n {
  let d = a.get_property(i);
  if Data::equals(d,b.clone()) { return i as i64; }
  i = i + 1;
}
-1
}

