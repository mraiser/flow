if (a.is_string()) { return a.string().len() as i64; }
else {
  return a.array().len() as i64
}