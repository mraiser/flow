use ndata::dataobject::DataObject;
use crate::flowlang::http::listen::*;
use std::io::Write;

pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["stream_id", "msg"] {
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
        let arg_0: i64 = o.get_int("stream_id");
        let arg_1: String = o.get_string("msg");
        websocket_write(arg_0, arg_1)
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

pub fn websocket_write(stream_id: i64, msg: String) -> i64 {
let msg = msg.as_bytes();

let n = msg.len() as i64;
let mut reply: Vec<u8> = Vec::new();

reply.push(129); // Text = 129 / Binary = 130;

if n < 126 {
  reply.push((n & 0xFF) as u8);
}
else if n < 65536 {
  reply.push(126);
  reply.push(((n >> 8) & 0xFF) as u8);
  reply.push((n & 0xFF) as u8);
}
else {
  reply.push(127);
  reply.push(((n >> 56) & 0xFF) as u8);
  reply.push(((n >> 48) & 0xFF) as u8);
  reply.push(((n >> 40) & 0xFF) as u8);
  reply.push(((n >> 32) & 0xFF) as u8);
  reply.push(((n >> 24) & 0xFF) as u8);
  reply.push(((n >> 16) & 0xFF) as u8);
  reply.push(((n >> 8) & 0xFF) as u8);
  reply.push((n & 0xFF) as u8);
}

reply.extend_from_slice(msg);

let heap = &mut WEBSOCKS.write().unwrap();
let heap = heap.as_mut().unwrap();
let sock = &mut heap.get(stream_id as usize);
let _ = sock.0.write(&reply).unwrap();

n as i64

}
