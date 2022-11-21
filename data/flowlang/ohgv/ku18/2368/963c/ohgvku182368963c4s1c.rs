let mut a = DataArray::new();

for file in fs::read_dir(&path).unwrap() {
  let name = file.unwrap().file_name();
  a.push_string(&name.into_string().unwrap());
}

a