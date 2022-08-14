use ndata::dataobject::*;
use crate::generated::flowlang::http::listen::*;
use crate::sha1::*;
use crate::base64::*;
use std::io::Write;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_i64("stream_id");
let a1 = o.get_string("key");
let ax = websocket(a0, a1);
let mut o = DataObject::new();
o.put_i64("a", ax);
o
}

pub fn websocket(stream_id:i64, key:String) -> i64 {
let heap = &mut WEBSOCKS.get().write().unwrap();
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

