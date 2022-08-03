use ndata::dataobject::*;
use crate::datastore::*;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("lib");
let a1 = o.get_string("id");
let ax = read(a0, a1);
let mut o = DataObject::new();
o.put_object("a", ax);
o
}

pub fn read(lib:String, id:String) -> DataObject {
let store = DataStore::new();
store.get_data(&lib, &id)
}

