use ndata::dataobject::*;
use ndata::dataarray::*;
use std::fs;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("path");
let ax = list(a0);
let mut o = DataObject::new();
o.put_array("a", ax);
o
}

pub fn list(path:String) -> DataArray {
let mut a = DataArray::new();

for file in fs::read_dir(&path).unwrap() {
  let name = file.unwrap().file_name();
  a.push_string(&name.into_string().unwrap());
}

a
}

