let mut out = DataObject::new();

let mut command = command.clone();
let a = command.get_string(0);
command.remove_property(0);

let mut args = Vec::<String>::new();
for arg in command.objects() {
  args.push(arg.string());
}

let cmd = Command::new(&a)
  .args(args)
  .stderr(Stdio::piped())
  .stdout(Stdio::piped())
  .spawn()
  .expect("failed to execute process");

let output = cmd.wait_with_output().unwrap();
let result = std::str::from_utf8(&output.stdout).unwrap();
let error = std::str::from_utf8(&output.stderr).unwrap();

out.put_string("out", result);
out.put_string("err", error);

out
