use ndata::dataobject::*;
use ndata::data::*;
use std::fs::File;
use std::io;
use std::io::BufRead;
pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("path");
let ax = read_properties(a0);
let mut o = DataObject::new();
o.put_object("a", ax);
o
}

pub fn read_properties(mut path:String) -> DataObject {
let mut o = DataObject::new();
let file = File::open(path).unwrap();
let lines = io::BufReader::new(file).lines();
for line in lines {
  if let Ok(oneline) = line {
    if !oneline.starts_with("#") {
      let pair: Vec<_> = oneline.splitn(2, "=").collect();
      o.put_str(&pair[0], &pair[1]);
    }
  }
}
o
}

