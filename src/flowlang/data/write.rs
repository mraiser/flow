use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use crate::datastore::*;
use crate::flowlang::system::time::time;

pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["lib", "id", "data", "readers", "writers"] {
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
        let arg_1: String = o.get_string("id");
        let arg_2: DataObject = o.get_object("data");
        let arg_3: DataArray = o.get_array("readers");
        let arg_4: DataArray = o.get_array("writers");
        write(arg_0, arg_1, arg_2, arg_3, arg_4)
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

pub fn write(lib: String, id: String, data: DataObject, readers: DataArray, writers: DataArray) -> DataObject {
let store = DataStore::new();
let mut o = DataObject::new();
o.put_string("id", &id);
o.put_object("data", data);
o.put_string("username", "system");
o.put_int("time", time());  
o.put_array("readers", readers);
o.put_array("writers", writers);
store.set_data(&lib, &id, o.clone());
o

}
