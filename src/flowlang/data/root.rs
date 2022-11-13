use ndata::dataobject::*;
use crate::datastore::*;

pub fn execute(_o: DataObject) -> DataObject {
let ax = root();
let mut o = DataObject::new();
o.put_str("a", &ax);
o
}

pub fn root() -> String {
let store = DataStore::new();
store.root.canonicalize().unwrap().to_str().unwrap().to_string()

}

