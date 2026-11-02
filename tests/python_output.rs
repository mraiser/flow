//! print(), sys.stderr and logging from Python commands reach the process's
//! stdout/stderr as complete lines labelled with the command. One test in its
//! own binary: it redirects the process's fd 1 and 2 around each call.
//!
//!   embedded:   print -> stdout; sys.stderr and logging -> stderr
//!   subprocess: everything -> stderr (the child's stdout carries the result)

use flowlang::datastore::DataStore;
use flowlang::pycmd::{run, PyCall};
use ndata::dataobject::DataObject;
use std::path::{Path, PathBuf};

const EMBEDDED: bool = cfg!(feature = "python_runtime");

fn command(base: &Path, lib: &str, imports: &str, body: &str) -> PyCall {
  let dir = base.join(lib).join("ctl");
  std::fs::create_dir_all(&dir).unwrap();
  let mut src = format!("{}\n\ndef execute(args):\n    return cmd()\n\ndef cmd():\n", imports);
  for line in body.lines() { src.push_str(&format!("    {}\n", line)); }
  let file = dir.join("cmd.py");
  std::fs::write(&file, src).unwrap();
  PyCall {
    label: format!("{}:ctl:cmd", lib), base_dir: base.to_path_buf(), module: format!("{}.ctl.cmd", lib),
    file, func: "cmd".to_string(), returntype: "Any".to_string(), args: DataObject::new(),
  }
}

/// Runs the calls with fd 1 and 2 redirected to files; returns (stdout, stderr).
fn captured(root: &Path, calls: &[&PyCall]) -> (String, String) {
  captured_with(root, || for c in calls { run(c).unwrap(); })
}

fn captured_with(root: &Path, f: impl FnOnce()) -> (String, String) {
  let (out_p, err_p): (PathBuf, PathBuf) = (root.join("out.txt"), root.join("err.txt"));
  let out_f = std::fs::File::create(&out_p).unwrap();
  let err_f = std::fs::File::create(&err_p).unwrap();
  use std::os::unix::io::AsRawFd;
  unsafe {
    let (saved_out, saved_err) = (libc::dup(1), libc::dup(2));
    libc::dup2(out_f.as_raw_fd(), 1);
    libc::dup2(err_f.as_raw_fd(), 2);
    f();
    libc::dup2(saved_out, 1);
    libc::dup2(saved_err, 2);
    libc::close(saved_out);
    libc::close(saved_err);
  }
  (std::fs::read_to_string(out_p).unwrap(), std::fs::read_to_string(err_p).unwrap())
}

fn lines_with(text: &str, needle: &str) -> Vec<String> {
  text.lines().filter(|l| l.contains(needle)).map(|l| l.to_string()).collect()
}

#[test]
fn command_output_is_labelled_lines() {
  // Must be set before the interpreter starts (embedded) / is inherited by
  // the child (subprocess).
  std::env::set_var("FLOWLANG_PYTHON_LOG_LEVEL", "info");
  let root = std::env::temp_dir().join(format!("flowlang_python_output_{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&root);
  std::fs::create_dir_all(root.join("data")).unwrap();
  DataStore::init(root.join("data").to_str().unwrap());
  let base = root.join("src");

  let talk = command(&base, "talk", "import sys, logging, threading", r#"
print('hello')
print('par', end='')
print('tial')
print('two\nlines')
sys.stderr.write('to stderr\n')
logging.getLogger('mylog').info('an info')
logging.getLogger('mylog').debug('a debug')
logging.warning('a warning')
t = threading.Thread(target=lambda: print('from a thread'))
t.start()
t.join()
print('unfinished', end='')
return 1"#);
  let next = command(&base, "next", "", "print('next command')\nreturn 1");
  let (out, err) = captured(&root, &[&talk, &next]);
  let (o, e) = if EMBEDDED { (&out, &err) } else { (&err, &err) };

  let t = "[py talk:ctl:cmd] ";
  for want in ["hello", "partial", "two", "lines"] {
    assert!(o.lines().any(|l| l == format!("{}{}", t, want)), "stdout lacks {:?}:\n{}", want, o);
  }
  assert!(e.lines().any(|l| l == format!("{}to stderr", t)), "{}", e);
  assert!(e.lines().any(|l| l == format!("{}INFO mylog: an info", t)), "{}", e);
  assert!(e.lines().any(|l| l == format!("{}WARNING root: a warning", t)), "{}", e);
  assert!(lines_with(e, "a debug").is_empty(), "debug is below the INFO level:\n{}", e);
  assert!(o.lines().any(|l| l == "[py] from a thread"), "{}", o);
  // A partial line is flushed with the command that wrote it.
  assert!(o.lines().any(|l| l == format!("{}unfinished", t)), "{}", o);
  assert!(o.lines().any(|l| l == "[py next:ctl:cmd] next command"), "{}", o);
  if EMBEDDED {
    assert!(lines_with(&err, "hello").is_empty(), "print went to stderr:\n{}", err);
    assert!(lines_with(&out, "to stderr").is_empty(), "stderr went to stdout:\n{}", out);
  }

  // Concurrent commands: every line carries its own command's label, whole.
  let workers: Vec<PyCall> = (0..4).map(|i| command(&base, &format!("conc{}", i), "",
    &format!("for n in range(200):\n    print('conc{} line', n)\nreturn 1", i))).collect();
  let (out, err) = captured_with(&root, || {
    std::thread::scope(|s| { for w in &workers { s.spawn(move || run(w).unwrap()); } });
  });
  let o = if EMBEDDED { &out } else { &err };
  let conc = lines_with(o, "conc");
  assert_eq!(conc.len(), 800, "{}", o);
  for l in &conc {
    let ok = (0..4).any(|i| l.starts_with(&format!("[py conc{}:ctl:cmd] conc{} line ", i, i)));
    assert!(ok, "mislabelled or mixed line: {:?}", l);
  }
  let _ = std::fs::remove_dir_all(&root);
}
