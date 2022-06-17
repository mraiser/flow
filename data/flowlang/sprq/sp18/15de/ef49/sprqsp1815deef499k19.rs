thread::spawn(move || {
  let cmd = Command::lookup(&lib, &ctl, &cmd);
  let _x = cmd.execute(params).unwrap();
});
1