let store = DataStore::new();
let path = store.get_data_file(&lib, "tasklists");
Path::new(&path).parent().unwrap().exists()