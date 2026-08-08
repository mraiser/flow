use ndata::dataobject::DataObject;

pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["input"] {
        if !o.has(p) {
            let mut e = DataObject::new();
            e.put_string("status", "err");
            e.put_string("msg", &format!("missing required parameter: {}", p));
            let mut result_obj = DataObject::new();
            result_obj.put_object("a", e);
            return result_obj;
        }
    }
    let ax = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let arg_0: String = o.get_string("input");
        hex_decode(arg_0)
    }));
    match ax {
        Ok(ax) => {
            let mut result_obj = DataObject::new();
    result_obj.put_string("a", &ax);
            result_obj
        }
        Err(err) => {
            let mut err_obj = DataObject::new();
            err_obj.put_string("status", "err");

            let msg = if let Some(s) = err.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = err.downcast_ref::<String>() {
                s.clone()
            } else {
                "Unknown panic occurred".to_string()
            };

            err_obj.put_string("msg", &msg);
            // Wrapped in the same `a` envelope a successful return uses.
            // Unwrapped, callers that unpack the envelope (newbound's
            // format_result, for one) report an opaque 500 — "Not an object:
            // DString(\"err\")" — instead of this message.
            let mut result_obj = DataObject::new();
            result_obj.put_object("a", err_obj);
            result_obj
        }
    }
}

pub fn hex_decode(input: String) -> String {
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
