use ndata::dataobject::DataObject;
use crate::command::*;

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
        execute_command(arg_0, arg_1, arg_2, arg_3)
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

pub fn execute_command(lib: String, ctl: String, cmd: String, params: DataObject) -> DataObject {
let cmd = Command::lookup(&lib, &ctl, &cmd);
let ret = cmd.return_type.to_owned();
let o = cmd.execute(params).unwrap();

if ret == "FLAT" { return o; }

let key;
if o.has("data") { key = "data".to_string(); }
else if o.has("msg") { key = "msg".to_string(); }
else if o.has("a") { key = "a".to_string(); }
else {
  let params = o.clone().keys();
  if params.len() == 0 { 
    return o; 
  }
  key = params[0].to_owned();
}
let val = o.get_property(&key);
let mut o = DataObject::new();
if ret == "String" { o.set_property("msg", val); }
else { o.set_property("data", val); }
o.put_string("status", "ok");
o

}
