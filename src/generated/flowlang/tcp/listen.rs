use ndata::dataobject::*;
use ndata::data::*;
use std::net::TcpListener;
use std::sync::RwLock;
use state::Storage;
use std::sync::Once;

use ndata::heap::Heap;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("address");
let a1 = o.get_i64("port");
let ax = listen(a0, a1);
let mut o = DataObject::new();
o.put_i64("a", ax);
o
}

pub fn listen(mut address:String, mut port:i64) -> i64 {
START.call_once(|| {
  TCPHEAP.set(RwLock::new(Heap::new()));
});

let socket_address = address + ":" + &port.to_string();
let listener = TcpListener::bind(socket_address).unwrap();
listener.set_nonblocking(true);
let data_ref = &mut TCPHEAP.get().write().unwrap().push(listener);

*data_ref as i64
}

static START: Once = Once::new();
pub static TCPHEAP:Storage<RwLock<Heap<TcpListener>>> = Storage::new();

fn xxx() {
}

