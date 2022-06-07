START.call_once(|| {
  STREAMHEAP.set(RwLock::new(Heap::new()));
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