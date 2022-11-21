use ndata::dataobject::*;
use ndata::dataarray::DataArray;
use crate::datastore::*;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("lib");
let a1 = o.get_string("ctl");
let a2 = o.get_string("cmd");
let a3 = o.get_object("params");
let ax = cast_params(a0, a1, a2, a3);
let mut o = DataObject::new();
o.put_object("a", ax);
o
}

pub fn cast_params(lib:String, ctl:String, cmd:String, params:DataObject) -> DataObject {
let store = DataStore::new();
let id = store.lookup_cmd_id(&lib, &ctl, &cmd);
let mut src = store.get_data(&lib, &id);
src = src.get_object("data");
let typ = src.get_string("type");
let id = src.get_string(&typ);
let mut src = store.get_data(&lib, &id);
src = src.get_object("data");
let list = src.get_array("params");
let mut outparams = DataObject::new();
for param in list.objects() {
  let p = param.object();
  let t = p.get_string("type");
  let n = p.get_string("name");
  if t == "Integer" { outparams.put_int(&n, params.get_string(&n).parse::<i64>().unwrap()); }
  else if t == "Float" { outparams.put_float(&n, params.get_string(&n).parse::<f64>().unwrap()); }
  else if t == "Boolean" { outparams.put_boolean(&n, params.get_string(&n).parse::<bool>().unwrap()); }
  else if t == "JSONObject" { outparams.put_object(&n, DataObject::from_string(&params.get_string(&n))); }
  else if t == "JSONArray" { outparams.put_array(&n, DataArray::from_string(&params.get_string(&n))); }
  else { outparams.put_string(&n, &params.get_string(&n)); }
}
outparams

}

