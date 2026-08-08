// Match JavaScript's encodeURIComponent: percent-encode the UTF-8 BYTES of
// every character outside the unreserved set (A-Za-z0-9 - _ . ! ~ * ' ( )),
// two uppercase hex digits per byte. The old encoder wrote the CODE POINT
// with unpadded width, so every non-ASCII character produced output nothing
// could decode ("é" -> "%E9", "—" -> "%2014", "\t" -> "%9") and distinct
// inputs could collide ("\t"+"a" -> "%9a", U+009A -> "%9A").
let mut s = String::new();
for b in a.bytes() {
  if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
    s.push(b as char);
  }
  else {
    s += &format!("%{:02X}", b);
  }
}
s