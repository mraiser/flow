use ndata::dataobject::*;
use crate::datastore::*;
use std::path::Path;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("lib");
let a1 = o.get_string("id");
let ax = exists(a0, a1);
let mut o = DataObject::new();
o.put_bool("a", ax);
o
}

pub fn exists(lib:String, id:String) -> bool {
let store = DataStore::new();
let path = store.get_data_file(&lib, &id);
Path::new(&path).exists()
}

