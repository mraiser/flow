let store = DataStore::new();
let mut o = DataObject::new();
o.put_string("id", &id);
o.put_object("data", data);
o.put_string("username", "system");
o.put_int("time", time());  
o.put_array("readers", readers);
o.put_array("writers", writers);
store.set_data(&lib, &id, o.clone());
o
