use ndata::dataobject::*;
use std::net::TcpListener;
use std::sync::RwLock;
//use state::Storage;
use std::sync::Once;

use ndata::heap::Heap;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("address");
let a1 = o.get_int("port");
let ax = listen(a0, a1);
let mut o = DataObject::new();
o.put_int("a", ax);
o
}

pub fn listen(address:String, port:i64) -> i64 {
START.call_once(|| {
  *TCPHEAP.write().unwrap() = Some(Heap::new());
  xxx();
});

let socket_address = address + ":" + &port.to_string();
let listener = TcpListener::bind(socket_address).unwrap();
let _ = listener.set_nonblocking(true).unwrap();
let data_ref = &mut TCPHEAP.write().unwrap();
let data_ref = data_ref.as_mut().unwrap();
let data_ref = data_ref.push(listener);
data_ref as i64
}

static START: Once = Once::new();
//pub static TCPHEAP:Storage<RwLock<Heap<TcpListener>>> = Storage::new();
pub static TCPHEAP:RwLock<Option<Heap<TcpListener>>> = RwLock::new(None);

fn xxx() {
}

