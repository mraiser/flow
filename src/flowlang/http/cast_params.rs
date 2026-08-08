use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use crate::datastore::*;

pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["lib", "ctl", "cmd", "params"] {
        if !o.has(p) {
            let mut e = DataObject::new();
            e.put_string("status", "err");
            e.put_string("msg", &format!("missing required parameter: {}", p));
            let mut result_obj = DataObject::new();
            result_obj.put_object("a", e);
            return result_obj;
        }
    }
    let ax = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let arg_0: String = o.get_string("lib");
        let arg_1: String = o.get_string("ctl");
        let arg_2: String = o.get_string("cmd");
        let arg_3: DataObject = o.get_object("params");
        cast_params(arg_0, arg_1, arg_2, arg_3)
    }));
    match ax {
        Ok(ax) => {
            let mut result_obj = DataObject::new();
    result_obj.put_object("a", ax);
            result_obj
        }
        Err(err) => {
            let mut err_obj = DataObject::new();
            err_obj.put_string("status", "err");

            let msg = if let Some(s) = err.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = err.downcast_ref::<String>() {
                s.clone()
            } else {
                "Unknown panic occurred".to_string()
            };

            err_obj.put_string("msg", &msg);
            // Wrapped in the same `a` envelope a successful return uses.
            // Unwrapped, callers that unpack the envelope (newbound's
            // format_result, for one) report an opaque 500 — "Not an object:
            // DString(\"err\")" — instead of this message.
            let mut result_obj = DataObject::new();
            result_obj.put_object("a", err_obj);
            result_obj
        }
    }
}

pub fn cast_params(lib: String, ctl: String, cmd: String, params: DataObject) -> DataObject {
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
