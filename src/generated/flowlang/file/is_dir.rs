use ndata::dataobject::*;
use ndata::data::*;
use std::fs::metadata;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("path");
let ax = is_dir(a0);
let mut o = DataObject::new();
o.put_bool("a", ax);
o
}

pub fn is_dir(mut path:String) -> bool {
let md = metadata(&path).unwrap();
md.is_dir()
}

