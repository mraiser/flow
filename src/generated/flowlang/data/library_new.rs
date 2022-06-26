use ndata::dataobject::*;
use ndata::data::*;
use std::path::Path;
use std::fs;

use ndata::dataarray::*;

use crate::datastore::*;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("lib");
let a1 = o.get_array("readers");
let a2 = o.get_array("writers");
let ax = library_new(a0, a1, a2);
let mut o = DataObject::new();
o.put_i64("a", ax);
o
}

pub fn library_new(mut lib:String, mut readers:DataArray, mut writers:DataArray) -> i64 {
let store = DataStore::new();
let mut path = store.root.join(lib);
if !path.exists() { fs::create_dir_all(&path); }

let mut meta = DataObject::new();
meta.put_str("username", "system");
meta.put_array("readers", readers);
meta.put_array("writers", writers);

path = path.join("meta.json");
fs::write(path, meta.to_json().to_string()).expect("Unable to write file");

// FIXME
// fireEvent("newdb", meta);

1
}

