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
