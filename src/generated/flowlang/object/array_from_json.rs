use ndata::dataobject::*;
use ndata::data::*;
use ndata::dataarray::DataArray;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("a");
let ax = array_from_json(a0);
let mut o = DataObject::new();
o.put_list("a", ax);
o
}

pub fn array_from_json(mut a:String) -> DataArray {
let v = serde_json::from_str(&a).unwrap();
DataArray::from_json(v)
}

