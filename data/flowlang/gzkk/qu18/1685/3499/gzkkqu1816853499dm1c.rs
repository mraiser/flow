let store = DataStore::new();
let mut path = store.root.join(lib);
if !path.exists() { let _ = fs::create_dir_all(&path).unwrap(); }

let mut meta = DataObject::new();
meta.put_string("username", "system");
meta.put_array("readers", readers);
meta.put_array("writers", writers);

path = path.join("meta.json");
fs::write(path, meta.to_string()).expect("Unable to write file");

// FIXME
// fireEvent("newdb", meta);

1
