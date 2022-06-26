use ndata::dataobject::*;
use ndata::data::*;
use ndata::dataarray::DataArray;
pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_property("a");
let a1 = o.get_property("b");
let ax = test_rust(a0, a1);
let mut o = DataObject::new();
o.set_property("a", ax);
o
}

pub fn test_rust(mut a:Data, mut b:Data) -> Data {
let mut ax = DataArray::new();
ax.push_property(a);
ax.push_property(b);
Data::DArray(ax.data_ref)
}

