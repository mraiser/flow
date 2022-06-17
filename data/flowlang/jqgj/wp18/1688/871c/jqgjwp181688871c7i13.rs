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