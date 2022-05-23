if a.is_object(){
  return a.object().get_property(&b.string());
}
else if a.is_array() {
  return a.array().get_property(b.int() as usize);
}
else {
  panic!("The get operation is not supported for this type ({:?})", a);
}