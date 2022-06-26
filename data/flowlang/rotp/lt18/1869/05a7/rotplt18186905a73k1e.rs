let mut a = DataArray::new();

for file in fs::read_dir(&path).unwrap() {
  let path = file.unwrap().path();
  let name = &path.display().to_string();
  let mut args = DataObject::new();
  args.put_str("path", &name);
  let o = execute_command(lib.to_owned(), ctl.to_owned(), cmd.to_owned(), args);
  if o.has("a") {
    a.push_property(o.get_property("a"));
  }
  
  if recursive && path.is_dir() {
    let a2 = visit(name.to_string(), recursive, lib.to_owned(), ctl.to_owned(), cmd.to_owned());
    a.join(a2);
  }
}

a