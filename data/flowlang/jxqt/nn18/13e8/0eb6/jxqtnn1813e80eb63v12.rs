START.call_once(|| {
  STREAMHEAP.set(RwLock::new(Heap::new()));
  xxx();
});

let mut o = DataObject::new();

let heap = &mut TCPHEAP.get().write().unwrap();
let l = heap.get(listener as usize);
let stream = l.accept();
if stream.is_err() {
  o.put_string("error", &format!("{:?}", stream));
}
else {
  let (s, a) = stream.unwrap();
  let data_ref = &mut STREAMHEAP.get().write().unwrap().push(s);
  o.put_int("stream", *data_ref as i64);
  o.put_string("address", &a.to_string());
}
o
}

static START: Once = Once::new();
pub static STREAMHEAP:Storage<RwLock<Heap<TcpStream>>> = Storage::new();

fn xxx() {