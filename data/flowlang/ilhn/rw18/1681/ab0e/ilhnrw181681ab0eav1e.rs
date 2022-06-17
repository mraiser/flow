thread::spawn(move || {
  let cmd = Command::new(&lib, &id);
  let _x = cmd.execute(params).unwrap();
});
1