let mut s = "".to_string();
let chars = a.chars();
for c in chars {
  if is_ok(c) {
    s.push(c);
  }
  else {
    let x = c as i32;
    s += "%";
    s += &format!( "{:0X}", x);
  }
}
s
}

fn is_ok(c:char) -> bool {
  (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9')

