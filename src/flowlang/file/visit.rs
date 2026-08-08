use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use std::fs;

use crate::flowlang::system::execute_command::*;
pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["path", "recursive", "lib", "ctl", "cmd"] {
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
        let arg_0: String = o.get_string("path");
        let arg_1: bool = o.get_boolean("recursive");
        let arg_2: String = o.get_string("lib");
        let arg_3: String = o.get_string("ctl");
        let arg_4: String = o.get_string("cmd");
        visit(arg_0, arg_1, arg_2, arg_3, arg_4)
    }));
    match ax {
        Ok(ax) => {
            let mut result_obj = DataObject::new();
    result_obj.put_array("a", ax);
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

pub fn visit(path: String, recursive: bool, lib: String, ctl: String, cmd: String) -> DataArray {
let mut a = DataArray::new();

for file in fs::read_dir(&path).unwrap() {
  let path = file.unwrap().path();
  let name = &path.display().to_string();
  let mut args = DataObject::new();
  args.put_string("path", &name);
  let o = execute_command(lib.to_owned(), ctl.to_owned(), cmd.to_owned(), args);
  if o.has("a") {
    a.push_property(o.get_property("a"));
  }
  
  if recursive && path.is_dir() {
    let a2 = visit(name.to_string(), recursive, lib.to_owned(), ctl.to_owned(), cmd.to_owned());
    a.join(a2);
  }
}

a
}
