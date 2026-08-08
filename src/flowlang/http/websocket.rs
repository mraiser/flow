use ndata::dataobject::DataObject;
use crate::flowlang::http::listen::*;
use crate::sha1::*;
use crate::base64::*;
use std::io::Write;

pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["stream_id", "key"] {
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
        let arg_1: String = o.get_string("key");
        websocket(arg_0, arg_1)
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

pub fn websocket(stream_id: i64, key: String) -> i64 {
let heap = &mut WEBSOCKS.write().unwrap();
let heap = heap.as_mut().unwrap();
let sock = &mut heap.get(stream_id as usize);
let stream = &mut sock.0;
let key = key.trim();
let key = key.to_string()+"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

let mut checksum = SHA1::new();
let _hash = checksum.update(&key);
let hash = checksum.finish();
let key2: String = Base64::encode(hash).into_iter().collect();

let mut response = "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n".to_string();
response += "Sec-WebSocket-Accept: ";
response += key2.trim();
response += "\r\n";
response += "Sec-WebSocket-Protocol: newbound\r\n\r\n";
stream.write(response.as_bytes()).unwrap();

stream_id

}
