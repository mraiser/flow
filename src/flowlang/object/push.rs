use ndata::dataobject::*;
use ndata::dataarray::DataArray;
use ndata::data::Data;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_array("a");
let a1 = o.get_property("b");
let ax = push(a0, a1);
let mut o = DataObject::new();
o.put_array("a", ax);
o
}

pub fn push(a:DataArray, b:Data) -> DataArray {
a.clone().push_property(b);
a
}

