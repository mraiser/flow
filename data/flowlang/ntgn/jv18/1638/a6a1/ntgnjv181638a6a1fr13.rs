if a.is_number() && b.is_number() {
  if a.is_float() {
    if b.is_float() {
      return a.float() > b.float();
    }
    return a.float() > (b.int() as f64);
  }
  if b.is_int() {
    return a.int() > b.int();
  }
  return (a.int() as f64) > b.float();
}
a.string().cmp(&b.string()).is_gt()