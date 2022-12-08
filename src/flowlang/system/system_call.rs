use ndata::dataobject::*;
use std::process::Command;
use std::process::Stdio;
use ndata::dataarray::DataArray;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_array("command");
let ax = system_call(a0);
let mut o = DataObject::new();
o.put_object("a", ax);
o
}

pub fn system_call(command:DataArray) -> DataObject {
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
  .spawn();

if cmd.is_err() {
  let msg = "Unable to execute system call ".to_string()+&a+" "+&command.to_string();
  println!("{}", msg);
  out.put_string("err", &msg);
}
else {
  let cmd = cmd.unwrap();
  let output = cmd.wait_with_output().unwrap();
  let result = std::str::from_utf8(&output.stdout).unwrap();
  let error = std::str::from_utf8(&output.stderr).unwrap();

  out.put_string("out", result);
  out.put_string("err", error);
}

out

}

