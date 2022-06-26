use ndata::dataobject::*;
use ndata::data::*;
use std::fs;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("path");
let ax = read_all_string(a0);
let mut o = DataObject::new();
o.put_str("a", &ax);
o
}

pub fn read_all_string(mut path:String) -> String {
fs::read_to_string(&path).unwrap()
}

