if a.is_object(){
  let a = a.object();
  let b = b.string();
  if a.has(&b) {
    return a.get_property(&b);
  }
  return Data::DNull;
}
else if a.is_array() {
  let a = a.array();
  let b = b.int() as usize;
  if b < a.len() {
    return a.get_property(b);
  }
  return Data::DNull;
}
panic!("The get operation is not supported for this type ({:?})", a);