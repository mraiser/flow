use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;

pub fn execute(_: DataObject) -> DataObject {
  let ax = list_resources();
  let mut result_obj = DataObject::new();
  result_obj.put_object("a", ax);
  result_obj
}

pub fn list_resources() -> DataObject {
let mut out = DataObject::new();
let resources = DataArray::new();

// Add resources here

out.put_array("resources", resources);
out
}
