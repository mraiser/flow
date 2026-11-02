//! When stdout carries a protocol (the MCP server's JSON-RPC), a Python
//! command's print() goes to stderr - labelled - and never to stdout. Its own
//! binary: the flag is set before the interpreter starts, and fds 1/2 are
//! redirected around the call.

use flowlang::datastore::DataStore;
use flowlang::pycmd::{run, PyCall};
use ndata::dataobject::DataObject;
use std::os::unix::io::AsRawFd;

#[test]
fn print_goes_to_stderr_when_stdout_is_a_protocol() {
  flowlang::pycmd::stdout_is_protocol();
  let root = std::env::temp_dir().join(format!("flowlang_py_protocol_{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&root);
  std::fs::create_dir_all(root.join("data")).unwrap();
  DataStore::init(root.join("data").to_str().unwrap());
  let base = root.join("src");
  let dir = base.join("proto").join("ctl");
  std::fs::create_dir_all(&dir).unwrap();
  let file = dir.join("cmd.py");
  std::fs::write(&file, "\n\ndef execute(args):\n    return cmd()\n\ndef cmd():\n    print('not for the protocol')\n    return 1\n").unwrap();
  let call = PyCall { label: "proto:ctl:cmd".into(), base_dir: base, module: "proto.ctl.cmd".into(), file,
                      func: "cmd".into(), returntype: "Any".into(), args: DataObject::new() };
  let out_f = std::fs::File::create(root.join("out.txt")).unwrap();
  let err_f = std::fs::File::create(root.join("err.txt")).unwrap();
  unsafe {
    let (o, e) = (libc::dup(1), libc::dup(2));
    libc::dup2(out_f.as_raw_fd(), 1);
    libc::dup2(err_f.as_raw_fd(), 2);
    run(&call).unwrap();
    libc::dup2(o, 1);
    libc::dup2(e, 2);
  }
  let out = std::fs::read_to_string(root.join("out.txt")).unwrap();
  let err = std::fs::read_to_string(root.join("err.txt")).unwrap();
  assert!(!out.contains("not for the protocol"), "stdout: {}", out);
  assert!(err.lines().any(|l| l == "[py proto:ctl:cmd] not for the protocol"), "stderr: {}", err);
  let _ = std::fs::remove_dir_all(&root);
}
