use ndata::dataobject::*;
use std::sync::RwLock;
use state::Storage;
use std::sync::Once;
use std::net::TcpStream;

use ndata::heap::Heap;

use crate::flowlang::tcp::listen::TCPHEAP;
pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_i64("listener");
let ax = accept(a0);
let mut o = DataObject::new();
o.put_object("a", ax);
o
}

pub fn accept(listener:i64) -> DataObject {
START.call_once(|| {
  STREAMHEAP.set(RwLock::new(Heap::new()));
  xxx();
});

let mut o = DataObject::new();

let heap = &mut TCPHEAP.get().write().unwrap();
let l = heap.get(listener as usize);
let stream = l.accept();
if stream.is_err() {
  o.put_str("error", &format!("{:?}", stream));
}
else {
  let (s, a) = stream.unwrap();
  let data_ref = &mut STREAMHEAP.get().write().unwrap().push(s);
  o.put_i64("stream", *data_ref as i64);
  o.put_str("address", &a.to_string());
}
o
}

static START: Once = Once::new();
pub static STREAMHEAP:Storage<RwLock<Heap<TcpStream>>> = Storage::new();

fn xxx() {
}

