use ndata::dataobject::*;
use ndata::data::*;
use ndata::dataarray::DataArray;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_array("a");
let a1 = o.get_array("b");
let ax = push_all(a0, a1);
let mut o = DataObject::new();
o.put_list("a", ax);
o
}

pub fn push_all(mut a:DataArray, mut b:DataArray) -> DataArray {
a.join(b);
a
}

