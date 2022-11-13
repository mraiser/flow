use ndata::dataobject::*;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("a");
let ax = hex_encode(a0);
let mut o = DataObject::new();
o.put_str("a", &ax);
o
}

pub fn hex_encode(a:String) -> String {
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


}

