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
  if t == "Integer" { outparams.put_i64(&n, params.get_string(&n).parse::<i64>().unwrap()); }
  else if t == "Float" { outparams.put_float(&n, params.get_string(&n).parse::<f64>().unwrap()); }
  else if t == "Boolean" { outparams.put_bool(&n, params.get_string(&n).parse::<bool>().unwrap()); }
  else if t == "JSONObject" { outparams.put_object(&n, DataObject::from_json(serde_json::from_str(&params.get_string(&n)).unwrap())); }
  else if t == "JSONArray" { outparams.put_list(&n, DataArray::from_json(serde_json::from_str(&params.get_string(&n)).unwrap())); }
  else { outparams.put_str(&n, &params.get_string(&n)); }
}
outparams