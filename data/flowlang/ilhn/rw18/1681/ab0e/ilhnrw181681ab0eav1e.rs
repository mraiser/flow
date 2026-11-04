thread::spawn(move || {
  let cmd = Command::new(&lib, &id);
  if let Err(e) = cmd.execute(params) { eprintln!("{}:{} (in a thread) {}", cmd.lib, cmd.name, e); }
});
1