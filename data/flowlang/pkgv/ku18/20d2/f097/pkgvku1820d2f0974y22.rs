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

