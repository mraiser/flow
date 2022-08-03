use ndata::dataobject::*;
use ndata::dataarray::DataArray;
use crate::datastore::*;
use crate::generated::flowlang::system::time::time;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("lib");
let a1 = o.get_string("id");
let a2 = o.get_object("data");
let a3 = o.get_array("readers");
let a4 = o.get_array("writers");
let ax = write(a0, a1, a2, a3, a4);
let mut o = DataObject::new();
o.put_i64("a", ax);
o
}

pub fn write(lib:String, id:String, data:DataObject, readers:DataArray, writers:DataArray) -> i64 {
let store = DataStore::new();
let mut o = DataObject::new();
o.put_str("id", &id);
o.put_object("data", data);
o.put_str("username", "system");
o.put_i64("time", time());  
o.put_array("readers", readers);
o.put_array("writers", writers);
store.set_data(&lib, &id, o);
1
}

