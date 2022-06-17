let mut i = 0;
let n = a.len();
while (i<n) {
  let d = a.get_property(i);
  if Data::equals(d,b.clone()) { return i as i64; }
  i = i + 1;
}
-1