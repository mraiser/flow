//! On a free-threaded CPython (3.13t/3.14t) Python commands run in
//! parallel: flowlang's modules declare they do not need the GIL, so
//! registering them does not re-enable it. With the GIL, CPU-bound commands
//! serialize (the test then only reports). Its own binary: a fresh
//! interpreter and a temporary store.
#![cfg(feature = "python_runtime")]

use flowlang::datastore::DataStore;
use flowlang::pycmd::{run, PyCall};
use ndata::dataobject::DataObject;
use std::time::Instant;

fn command(base: &std::path::Path, name: &str, body: &str) -> PyCall {
  let dir = base.join("par").join("ctl");
  std::fs::create_dir_all(&dir).unwrap();
  let mut src = format!("import sys, sysconfig\n\ndef execute(args):\n    return {}()\n\ndef {}():\n", name, name);
  for l in body.lines() { src.push_str(&format!("    {}\n", l)); }
  let file = dir.join(format!("{}.py", name));
  std::fs::write(&file, src).unwrap();
  PyCall { label: format!("par:ctl:{}", name), base_dir: base.to_path_buf(), module: format!("par.ctl.{}", name),
           file, func: name.to_string(), returntype: "Any".into(), args: DataObject::new() }
}

#[test]
fn cpu_bound_commands_run_in_parallel_without_the_gil() {
  let root = std::env::temp_dir().join(format!("flowlang_py_parallel_{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&root);
  std::fs::create_dir_all(root.join("data")).unwrap();
  DataStore::init(root.join("data").to_str().unwrap());
  let base = root.join("src");
  let info = command(&base, "info", "import ndata_py, flowlang\nreturn [bool(sysconfig.get_config_var('Py_GIL_DISABLED')), sys._is_gil_enabled() if hasattr(sys, '_is_gil_enabled') else True]");
  let work = command(&base, "work", "t = 0\nfor i in range(3_000_000):\n    t += i * i\nreturn t");
  let env = run(&info).unwrap();
  let flags = env.get_array("a");
  let (free_threaded, gil_enabled) = (flags.get_boolean(0), flags.get_boolean(1));
  if free_threaded {
    assert!(!gil_enabled, "the GIL was re-enabled - a module was imported without declaring gil_used = false");
  }
  run(&work).unwrap(); // warm up
  let n = 4;
  let t = Instant::now();
  for _ in 0..n { run(&work).unwrap(); }
  let sequential = t.elapsed();
  let t = Instant::now();
  std::thread::scope(|s| { for _ in 0..n { s.spawn(|| run(&work).unwrap()); } });
  let parallel = t.elapsed();
  let ratio = parallel.as_secs_f64() / sequential.as_secs_f64();
  eprintln!("free-threaded: {}, GIL enabled: {}; {} calls sequential {:?}, parallel {:?} (ratio {:.2})",
            free_threaded, gil_enabled, n, sequential, parallel, ratio);
  let cores = std::thread::available_parallelism().map(|c| c.get()).unwrap_or(1);
  if free_threaded && cores >= n {
    assert!(ratio < 0.6, "no parallel speedup on a free-threaded build: ratio {:.2}", ratio);
  }
  let _ = std::fs::remove_dir_all(&root);
}
