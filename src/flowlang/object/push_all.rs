use ndata::dataobject::*;
use ndata::dataarray::DataArray;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_array("a");
let a1 = o.get_array("b");
let ax = push_all(a0, a1);
let mut o = DataObject::new();
o.put_array("a", ax);
o
}

pub fn push_all(a:DataArray, b:DataArray) -> DataArray {
a.duplicate().join(b);
a
}

