use ndata::dataobject::*;
use ndata::dataarray::*;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("a");
let a1 = o.get_string("b");
let ax = split(a0, a1);
let mut o = DataObject::new();
o.put_array("a", ax);
o
}

pub fn split(a:String, b:String) -> DataArray {
let sa = a.split(&b);
let mut ja = DataArray::new();
for i in sa {
  ja.push_str(&i);
}
ja
}

