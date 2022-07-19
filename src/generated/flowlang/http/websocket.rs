use ndata::dataobject::*;
use ndata::data::*;
use crate::generated::flowlang::http::listen::*;
use sha1::{Sha1, Digest};
extern crate base64;
use std::io::Write;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_i64("stream_id");
let a1 = o.get_string("key");
let ax = websocket(a0, a1);
let mut o = DataObject::new();
o.put_i64("a", ax);
o
}

pub fn websocket(mut stream_id:i64, mut key:String) -> i64 {
let heap = &mut WEBSOCKS.get().write().unwrap();
let sock = &mut heap.get(stream_id as usize);
let stream = &mut sock.0;
let key = key.trim();
let key = key.to_string()+"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
let mut hasher = Sha1::new();
let hash = hasher.update(key.as_bytes());
let hash = hasher.finalize();
let key = base64::encode(&hash);

let mut response = "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n".to_string();
response += "Sec-WebSocket-Accept: ";
response += key.trim();
response += "\r\n";
response += "Sec-WebSocket-Protocol: newbound\r\n\r\n";
stream.write(response.as_bytes()).unwrap();

stream_id

}

