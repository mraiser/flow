use ndata::dataobject::*;
use ndata::data::*;
use std::path::Path;
pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("path");
let ax = exists(a0);
let mut o = DataObject::new();
o.put_bool("a", ax);
o
}

pub fn exists(mut path:String) -> bool {
Path::new(&path).exists()
}

