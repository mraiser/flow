let mut old = input;
let mut out = "".to_string();
let mut i;

while old.contains("%") {
  i = old.find("%").unwrap();
  out = out + &old[0..1];
  old = old[i..].to_string();
  if old.len() < 3 {
    break;
  }
  
  let s = old[1..3].to_string();
  let res:Result<Vec<u8>, ParseIntError> = (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
        .collect();
  
  if res.is_err(){
    out = out + &old[0..1];
    old = old[1..].to_string();
  }
  else {
    out = out + std::str::from_utf8(&res.unwrap()).unwrap();
    old = old[3..].to_string();
  }
}

out + &old