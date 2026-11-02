//! Reference counts across the Python boundary: after a command's Python
//! objects are gone and GC has run, every ndata object it created is freed.
//! A separate test binary, so no other test's objects are live meanwhile.
#![cfg(feature = "python_runtime")]

use flowlang::datastore::DataStore;
use flowlang::pycmd::{run, PyCall};
use ndata::dataobject::DataObject;
use std::path::Path;

fn command(base: &Path, lib: &str, body: &str) -> PyCall {
  let dir = base.join(lib).join("ctl");
  std::fs::create_dir_all(&dir).unwrap();
  let mut src = String::from("import ndata_py\n\ndef execute(args):\n    return cmd()\n\ndef cmd():\n");
  for line in body.lines() { src.push_str(&format!("    {}\n", line)); }
  let file = dir.join("cmd.py");
  std::fs::write(&file, src).unwrap();
  PyCall {
    label: format!("{}:ctl:cmd", lib),
    base_dir: base.to_path_buf(),
    module: format!("{}.ctl.cmd", lib),
    file,
    func: "cmd".to_string(),
    returntype: "Any".to_string(),
    args: DataObject::new(),
  }
}

fn live_objects() -> usize {
  for _ in 0..3 { DataStore::gc(); }
  ndata::dataobject::oheap().lock().keys().len()
}

#[test]
fn no_object_outlives_its_last_handle() {
  let root = std::env::temp_dir().join(format!("flowlang_python_refcounts_{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&root);
  std::fs::create_dir_all(root.join("data")).unwrap();
  DataStore::init(root.join("data").to_str().unwrap());
  let base = root.join("src");
  let cases = [
    // pop: was +1 reference per popped container (pop_property's reference
    // was never released)
    ("pop", "a = ndata_py.NDataArray()\nfor _ in range(200):\n    a.append(ndata_py.NDataObject())\n    a.pop()\nreturn None"),
    ("setitem", "o = ndata_py.NDataObject()\nfor i in range(200):\n    o['k'] = {'n': i}\nreturn None"),
    ("convert", "return [{'n': i} for i in range(200)]"),
  ];
  for (name, body) in cases {
    let call = command(&base, &format!("rc_{}", name), body);
    run(&call).unwrap(); // warm up: import, interpreter state
    let before = live_objects();
    let env = run(&call).unwrap();
    drop(env);
    let after = live_objects();
    assert_eq!(after, before, "{}: {} ndata objects leaked", name, after as isize - before as isize);
  }
  let _ = std::fs::remove_dir_all(&root);
}
