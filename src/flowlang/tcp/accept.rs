use ndata::dataobject::*;
use std::sync::RwLock;
//use state::Storage;
use std::sync::Once;
use std::net::TcpStream;

use ndata::heap::Heap;

use crate::flowlang::tcp::listen::TCPHEAP;
pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_int("listener");
let ax = accept(a0);
let mut o = DataObject::new();
o.put_object("a", ax);
o
}

pub fn accept(listener:i64) -> DataObject {
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

