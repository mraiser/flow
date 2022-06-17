let store = DataStore::new();
let path = store.get_data_file(&lib, &id);
Path::new(&path).exists()