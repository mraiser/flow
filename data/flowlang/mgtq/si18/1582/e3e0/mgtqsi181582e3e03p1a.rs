let mut ja = DataArray::new();
for key in a.keys() {
  ja.push_string(&key);
}
ja