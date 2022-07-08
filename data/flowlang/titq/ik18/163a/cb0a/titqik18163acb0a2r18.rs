if a.is_array() {
  let mut aa = a.array();
  let b = b.int() as usize;
  aa.remove_property(b)
}
else {
  let mut aa = a.object();
  let b = b.string();
  aa.remove_property(&b);
}
a