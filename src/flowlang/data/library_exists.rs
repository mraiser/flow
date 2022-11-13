use ndata::dataobject::*;
use crate::datastore::*;
use std::path::Path;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("lib");
let ax = library_exists(a0);
let mut o = DataObject::new();
o.put_bool("a", ax);
o
}

pub fn library_exists(lib:String) -> bool {
let store = DataStore::new();
let path = store.get_data_file(&lib, "tasklists");
Path::new(&path).parent().unwrap().exists()
}

