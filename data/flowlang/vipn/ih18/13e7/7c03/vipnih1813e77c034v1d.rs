START.call_once(|| {
  TCPHEAP.set(RwLock::new(Heap::new()));
  xxx();
});

let socket_address = address + ":" + &port.to_string();
let listener = TcpListener::bind(socket_address).unwrap();
let _ = listener.set_nonblocking(true).unwrap();
let data_ref = &mut TCPHEAP.get().write().unwrap().push(listener);

*data_ref as i64
}

static START: Once = Once::new();
pub static TCPHEAP:Storage<RwLock<Heap<TcpListener>>> = Storage::new();

fn xxx() {