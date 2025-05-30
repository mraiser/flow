use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;

pub fn execute(_: DataObject) -> DataObject {
  let ax = list_prompts();
  let mut result_obj = DataObject::new();
  result_obj.put_object("a", ax);
  result_obj
}

pub fn list_prompts() -> DataObject {
let mut out = DataObject::new();
let prompts = DataArray::new();

// Add prompts here

out.put_array("prompts", prompts);
out
}
