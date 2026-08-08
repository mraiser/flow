use ndata::dataobject::DataObject;
use ndata::data::Data;

pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["a", "b"] {
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
        let arg_0: Data = o.get_property("a");
        let arg_1: Data = o.get_property("b");
        divide(arg_0, arg_1)
    }));
    match ax {
        Ok(ax) => {
            let mut result_obj = DataObject::new();
    result_obj.set_property("a", ax);
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

pub fn divide(a: Data, b: Data) -> Data {
if a.is_number() && b.is_number() {
  if a.is_float() || b.is_float() { 
    let c = a.float() / b.float();
    return Data::DFloat(c); 
  }
  else {
    let c = a.int() / b.int();
    return Data::DInt(c);
  }
}  
else {
  return Data::DString("NaN".to_owned());
}

}
