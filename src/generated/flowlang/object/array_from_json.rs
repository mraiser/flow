use ndata::dataobject::*;
use ndata::dataarray::DataArray;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("a");
let ax = array_from_json(a0);
let mut o = DataObject::new();
o.put_array("a", ax);
o
}

pub fn array_from_json(a:String) -> DataArray {
DataArray::from_string(&a)

}

