let sa = a.split(&b);
let mut ja = DataArray::new();
for i in sa {
  ja.push_string(&i);
}
ja