use ndata::dataobject::*;
use ndata::data::*;
use ndata::dataarray::DataArray;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_object("a");
let ax = keys(a0);
let mut o = DataObject::new();
o.put_list("a", ax);
o
}

pub fn keys(mut a:DataObject) -> DataArray {
let mut ja = DataArray::new();
for key in a.keys() {
  ja.push_str(&key);
}
ja
}

