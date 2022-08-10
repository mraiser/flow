if a.is_array() {
  let a = a.array();
  let mut i = 0;
  let n = a.len();
  while i<n {
    let d = a.get_property(i);
    if Data::equals(d,b.clone()) { return i as i64; }
    i = i + 1;
  }
}
else {
  let a = a.string();
  let i = a.find(&b.string());
  if i.is_some() { return i.unwrap() as i64; }
}
-1
