use ndata::dataobject::*;
use ndata::data::*;
use ndata::dataarray::DataArray;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_array("a");
let a1 = o.get_property("b");
let ax = push(a0, a1);
let mut o = DataObject::new();
o.put_list("a", ax);
o
}

pub fn push(mut a:DataArray, mut b:Data) -> DataArray {
a.push_property(b);
a
}

