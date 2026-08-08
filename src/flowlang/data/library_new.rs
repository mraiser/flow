use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use std::fs;


use crate::datastore::*;

pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["lib", "readers", "writers"] {
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
        let arg_1: DataArray = o.get_array("readers");
        let arg_2: DataArray = o.get_array("writers");
        library_new(arg_0, arg_1, arg_2)
    }));
    match ax {
        Ok(ax) => {
            let mut result_obj = DataObject::new();
    result_obj.put_int("a", ax);
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

pub fn library_new(lib: String, readers: DataArray, writers: DataArray) -> i64 {
let store = DataStore::new();
let mut path = store.root.join(lib);
if !path.exists() { let _ = fs::create_dir_all(&path).unwrap(); }

let mut meta = DataObject::new();
meta.put_string("username", "system");
meta.put_array("readers", readers);
meta.put_array("writers", writers);

path = path.join("meta.json");
fs::write(path, meta.to_string()).expect("Unable to write file");

// FIXME
// fireEvent("newdb", meta);

1

}
