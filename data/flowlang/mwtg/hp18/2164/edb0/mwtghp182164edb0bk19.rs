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
