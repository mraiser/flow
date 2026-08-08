use ndata::dataobject::DataObject;
use std::sync::RwLock;
use std::sync::Once;
use std::net::TcpStream;

use ndata::heap::Heap;

use crate::flowlang::tcp::listen::TCPHEAP;
pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["listener"] {
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
        let arg_0: i64 = o.get_int("listener");
        accept(arg_0)
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

pub fn accept(listener: i64) -> DataObject {
START.call_once(|| {
  *STREAMHEAP.write().unwrap() = Some(Heap::new());
  xxx();
});

let mut o = DataObject::new();

let l;
{
    let heap = &mut TCPHEAP.write().unwrap();
    let heap = heap.as_mut().unwrap();
    l = heap.get(listener as usize).try_clone().unwrap();
}
let stream = l.accept();
if stream.is_err() {
  o.put_string("error", &format!("{:?}", stream));
}
else {
  let (s, a) = stream.unwrap();
  let data_ref;
  {
    let x = &mut STREAMHEAP.write().unwrap();
    let x = x.as_mut().unwrap();
    data_ref = x.push(s);
  }
  o.put_int("stream", data_ref as i64);
  o.put_string("address", &a.to_string());
}
o
}

static START: Once = Once::new();
//pub static STREAMHEAP:Storage<RwLock<Heap<TcpStream>>> = Storage::new();
pub static STREAMHEAP:RwLock<Option<Heap<TcpStream>>> = RwLock::new(None);

fn xxx() {

}
