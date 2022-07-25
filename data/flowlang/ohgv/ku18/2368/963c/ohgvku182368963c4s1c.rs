let mut a = DataArray::new();

for file in fs::read_dir(&path).unwrap() {
  let name = file.unwrap().file_name();
  a.push_str(&name.into_string().unwrap());
}

a