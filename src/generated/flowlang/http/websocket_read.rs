use ndata::dataobject::*;
use crate::generated::flowlang::http::listen::*;
use std::io::Read;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_i64("stream_id");
let ax = websocket_read(a0);
let mut o = DataObject::new();
o.put_str("a", &ax);
o
}

pub fn websocket_read(stream_id:i64) -> String {
let mut reader;
{
  let heap = &mut WEBSOCKS.get().write().unwrap();
  let sock = &mut heap.get(stream_id as usize);
  reader = sock.0.try_clone().unwrap();
}

let base:i64 = 2;
let pow7 = base.pow(7);
let mut lastopcode = 0;
let mut baos: Vec<u8> = Vec::new();

loop {
  let mut buf = [0; 1];
  let _ = reader.read_exact(&mut buf).unwrap();
  let i = buf[0] as i64;
  let fin = (pow7 & i) != 0;
  let rsv1 = (base.pow(6) & i) != 0;
  let rsv2 = (base.pow(5) & i) != 0;
  let rsv3 = (base.pow(4) & i) != 0;

  if rsv1 || rsv2 || rsv3 { panic!("Websocket failed - Unimplimented"); } 

  let mut opcode = 0xf & i;

  let _ = reader.read_exact(&mut buf).unwrap();
  let i = buf[0] as i64;
  let mask = (pow7 & i) != 0;
  if !mask { panic!("Websocket failed - Mask required"); } 

  let mut len = i - pow7;

  if len == 126 {
    let mut buf = [0; 2];
    let _ = reader.read_exact(&mut buf).unwrap();
    len = (buf[0] as i64 & 0x000000FF) << 8;
    len += buf[1] as i64 & 0x000000FF;
  }
  else if len == 127 {
    let mut buf = [0; 8];
    let _ = reader.read_exact(&mut buf).unwrap();
    len = (buf[0] as i64 & 0x000000FF) << 56;
    len += (buf[1] as i64 & 0x000000FF) << 48;
    len += (buf[2] as i64 & 0x000000FF) << 40;
    len += (buf[3] as i64 & 0x000000FF) << 32;
    len += (buf[4] as i64 & 0x000000FF) << 24;
    len += (buf[5] as i64 & 0x000000FF) << 16;
    len += (buf[6] as i64 & 0x000000FF) << 8;
    len += buf[7] as i64 & 0x000000FF;
  }

  // FIXME - Should read larger messages in chunks
  // if len > 4096 { panic!("Websocket message too long ({})", len); } 
  let len = len as usize;

  let mut maskkey = [0; 4];
  let _ = reader.read_exact(&mut maskkey).unwrap();

  let mut buf = vec![0; len as usize];
  let _ = reader.read_exact(&mut buf).unwrap();
  let mut i:usize = 0;
  while i < len {
    buf[i] = buf[i] ^ maskkey[i % 4];
    i += 1;
  }
  
  baos.append(&mut buf);

  if opcode == 0 {
    println!("continuation frame");
  }
  else if opcode == 1 || opcode == 2 { lastopcode = opcode; }
  else if opcode == 8 {  panic!("Websocket closed"); } 
  else if opcode == 9 {
    println!("ping");
  }
  else if opcode == 10 {
    println!("pong");
  }
  else {
    println!("UNEXPECTED OPCODE: {}", opcode);
  }

  if fin {
    if opcode == 0 {
      opcode = lastopcode;
    }
    
    if opcode == 1 {
      // text frame
      break;
    }
    else if opcode == 2 {
      // binary frame
      // FIXME - passing text anyway.
      break;
    }
  }
}

let msg = std::str::from_utf8(&baos).unwrap();
//println!("{}", msg);

msg.to_string()
}

