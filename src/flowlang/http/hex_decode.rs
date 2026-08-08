use ndata::dataobject::*;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("input");
let ax = hex_decode(a0);
let mut o = DataObject::new();
o.put_string("a", &ax);
o
}

pub fn hex_decode(input:String) -> String {
// Percent-decoding is BYTE-oriented: one non-ASCII character arrives as a
// RUN of consecutive %XX escapes (UTF-8 bytes), so escapes must be collected
// into a byte run before UTF-8 validation. Decoding each escape alone
// rejects every byte >= 0x80 and lets multi-byte characters through as
// literal "%E2%80%94" text. Invalid escapes and byte runs that are not
// valid UTF-8 fall back to the old per-escape behavior (literal
// passthrough), so no input that previously decoded changes meaning.
let src: Vec<char> = input.chars().collect();
let n = src.len();
let mut out = String::new();
let mut i = 0;

let hexval = |c: char| c.to_digit(16);

while i < n {
  if src[i] != '%' {
    out.push(src[i]);
    i += 1;
    continue;
  }
  // collect the run of consecutive well-formed %XX escapes starting here
  let start = i;
  let mut bytes: Vec<u8> = Vec::new();
  while i < n && src[i] == '%' && i + 2 < n {
    let h = hexval(src[i + 1]);
    let l = hexval(src[i + 2]);
    if h.is_none() || l.is_none() { break; }
    bytes.push((h.unwrap() * 16 + l.unwrap()) as u8);
    i += 3;
  }
  if bytes.is_empty() {
    // a bare or malformed '%' — literal, as before
    out.push('%');
    i = start + 1;
    continue;
  }
  match std::str::from_utf8(&bytes) {
    Ok(s) => out.push_str(s),
    Err(_) => {
      // not UTF-8 as a whole: the old escape-by-escape fallback
      for (j, b) in bytes.iter().enumerate() {
        match std::str::from_utf8(std::slice::from_ref(b)) {
          Ok(s) => out.push_str(s),
          Err(_) => {
            out.push('%');
            out.push(src[start + j * 3 + 1]);
            out.push(src[start + j * 3 + 2]);
          }
        }
      }
    }
  }
}

out
}
