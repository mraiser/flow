thread::spawn(move || {
  let cmd = Command::lookup(&lib, &ctl, &cmd);
  if let Err(e) = cmd.execute(params) { eprintln!("{}:{} (in a thread) {}", cmd.lib, cmd.name, e); }
});
1