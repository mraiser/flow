use ndata::dataobject::DataObject;
use crate::datastore::*;

use crate::flowlang::system::random_non_hex_char::random_non_hex_char;
use crate::flowlang::system::time::time;

pub fn execute(_: DataObject) -> DataObject {
    use std::panic;
    let ax = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        unique_session_id()
    }));
    match ax {
        Ok(ax) => {
            let mut result_obj = DataObject::new();
    result_obj.put_string("a", &ax);
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

pub fn unique_session_id() -> String {
let mut globals = DataStore::globals();
if !globals.has("last_session_index") { globals.put_int("last_session_index", 0); }
let last_id = globals.get_int("last_session_index");
let mut next_id = last_id + 1;
if next_id > 65535 { next_id = 0; }
globals.put_int("last_session_index", next_id);

let s = random_non_hex_char()
  + &random_non_hex_char()
  + &random_non_hex_char()
  + &random_non_hex_char()
  + &random_non_hex_char()
  + &random_non_hex_char()
  + &format!("{:x}", time())
  + &random_non_hex_char()
  + &format!("{:x}", last_id);

s
}
