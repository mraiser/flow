use ndata::dataobject::*;
use std::fs::File;
use std::io::Write;
use ndata::data::Data;
pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("path");
let a1 = o.get_object("data");
let ax = write_properties(a0, a1);
let mut o = DataObject::new();
o.put_string("a", &ax);
o
}

pub fn write_properties(path:String, data:DataObject) -> String {
let mut file = File::create(path).unwrap();
for (k,v) in data.objects() {
  let s = format!("{}={}\n",k,Data::as_string(v));
  file.write_all(s.as_bytes()).unwrap();
}
"OK".to_string()
}

