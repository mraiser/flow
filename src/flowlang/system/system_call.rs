use ndata::dataobject::DataObject;
use std::process::Command;
use std::process::Stdio;
use ndata::dataarray::DataArray;

pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["command"] {
        if !o.has(p) {
            let mut e = DataObject::new();
            e.put_string("status", "err");
            e.put_string("msg", &format!("missing required parameter: {}", p));
            let mut result_obj = DataObject::new();
            result_obj.put_object("a", e);
            return result_obj;
        }
    }
    let ax = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let arg_0: DataArray = o.get_array("command");
        system_call(arg_0)
    }));
    match ax {
        Ok(ax) => {
            let mut result_obj = DataObject::new();
    result_obj.put_object("a", ax);
            result_obj
        }
        Err(err) => {
            let mut err_obj = DataObject::new();
            err_obj.put_string("status", "err");

            let msg = if let Some(s) = err.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = err.downcast_ref::<String>() {
                s.clone()
            } else {
                "Unknown panic occurred".to_string()
            };

            err_obj.put_string("msg", &msg);
            // Wrapped in the same `a` envelope a successful return uses.
            // Unwrapped, callers that unpack the envelope (newbound's
            // format_result, for one) report an opaque 500 — "Not an object:
            // DString(\"err\")" — instead of this message.
            let mut result_obj = DataObject::new();
            result_obj.put_object("a", err_obj);
            result_obj
        }
    }
}

pub fn system_call(command: DataArray) -> DataObject {
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
  out.put_string("status", "err");
}
else {
  let cmd = cmd.unwrap();
  let output = cmd.wait_with_output().unwrap();
  let result = std::str::from_utf8(&output.stdout).unwrap();
  let error = std::str::from_utf8(&output.stderr).unwrap();

  out.put_string("status", "ok");
  out.put_string("out", result);
  out.put_string("err", error);
}

out

}
