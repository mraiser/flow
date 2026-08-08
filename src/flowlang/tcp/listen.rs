use ndata::dataobject::DataObject;
use std::net::TcpListener;
use std::sync::RwLock;
use std::sync::Once;

use ndata::heap::Heap;

pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["address", "port"] {
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
        let arg_0: String = o.get_string("address");
        let arg_1: i64 = o.get_int("port");
        listen(arg_0, arg_1)
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

pub fn listen(address: String, port: i64) -> i64 {
START.call_once(|| {
  *TCPHEAP.write().unwrap() = Some(Heap::new());
  xxx();
});

let socket_address = address + ":" + &port.to_string();
let listener = TcpListener::bind(socket_address).unwrap();
let _ = listener.set_nonblocking(true).unwrap();
let data_ref = &mut TCPHEAP.write().unwrap();
let data_ref = data_ref.as_mut().unwrap();
let data_ref = data_ref.push(listener);
data_ref as i64
}

static START: Once = Once::new();
//pub static TCPHEAP:Storage<RwLock<Heap<TcpListener>>> = Storage::new();
pub static TCPHEAP:RwLock<Option<Heap<TcpListener>>> = RwLock::new(None);

fn xxx() {

}
