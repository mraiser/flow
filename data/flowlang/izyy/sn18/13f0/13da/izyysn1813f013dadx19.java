if a.is_number() && b.is_number() {
  if a.is_float() || b.is_float() { 
    let c = a.float() * b.float();
    return Data::DFloat(c); 
  }
  else {
    let c = a.int() * b.int();
    return Data::DInt(c);
  }
}  
else {
  return Data::DString("NaN".to_owned());
}
