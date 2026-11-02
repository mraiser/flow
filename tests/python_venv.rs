//! The embedded runtime and a `venv` next to `data/`: the venv is configured
//! ONCE, when the interpreter starts. Before, every call spawned the venv's
//! python to re-read its sys.path (about 70x slower than a call without a
//! venv) and re-ran site.addsitedir.
//!
//! A separate test binary: the venv is found next to the DataStore root, and
//! the DataStore is initialized once per process.
#![cfg(all(unix, feature = "python_runtime"))]

use flowlang::datastore::DataStore;
use flowlang::pycmd::{run, PyCall};
use ndata::dataobject::DataObject;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// `$PYO3_PYTHON` (the interpreter the runtime was built against), else python3.
fn python() -> String {
  std::env::var("PYO3_PYTHON").unwrap_or_else(|_| "python3".to_string())
}

fn site_packages(venv: &Path) -> PathBuf {
  let lib = venv.join("lib");
  let ver = std::fs::read_dir(&lib).unwrap().next().unwrap().unwrap().file_name();
  lib.join(ver).join("site-packages")
}

fn command(base: &Path, lib: &str, body: &str) -> PyCall {
  let dir = base.join(lib).join("ctl");
  std::fs::create_dir_all(&dir).unwrap();
  let mut src = String::from("\n\ndef execute(args):\n    return cmd()\n\ndef cmd():\n");
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

#[test]
fn venv_is_configured_once_and_its_packages_import() {
  let root = std::env::temp_dir().join(format!("flowlang_python_venv_{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&root);
  std::fs::create_dir_all(root.join("data")).unwrap();
  let venv = root.join("venv");
  let ok = Command::new(python()).args(["-m", "venv", "--without-pip"]).arg(&venv).status().unwrap();
  assert!(ok.success(), "could not create a venv with {}", python());

  // A package installed in the venv, and an editable install (.pth file).
  let sp = site_packages(&venv);
  std::fs::write(sp.join("venvmod.py"), "VALUE = 'from venv'\n").unwrap();
  let editable = root.join("editable");
  std::fs::create_dir_all(&editable).unwrap();
  std::fs::write(editable.join("edmod.py"), "VALUE = 'from editable'\n").unwrap();
  std::fs::write(sp.join("_edmod.pth"), format!("{}\n", editable.display())).unwrap();

  // Count every launch of the venv's python.
  let counter = root.join("launches");
  let launcher = venv.join("bin").join("python");
  std::fs::remove_file(&launcher).unwrap();
  std::fs::write(&launcher, format!(
    "#!/bin/sh\necho launch >> '{}'\nexec \"$(dirname \"$0\")/python3\" \"$@\"\n", counter.display()
  )).unwrap();
  std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o755)).unwrap();

  DataStore::init(root.join("data").to_str().unwrap());
  let base = root.join("src");
  let call = command(&base, "venvlib", "import venvmod, edmod\nreturn [venvmod.VALUE, edmod.VALUE]");
  for _ in 0..25 {
    let d = run(&call).unwrap();
    let mut o = DataObject::new();
    o.set_property("v", d.get_property("a"));
    assert_eq!(o.to_string(), r#"{"v":["from venv","from editable"]}"#);
  }
  let launches = std::fs::read_to_string(&counter).unwrap_or_default().lines().count();
  assert_eq!(launches, 1, "the venv's python ran {} times for 25 calls", launches);

  // The venv's '' (from `python -c`) did not come back.
  let paths = run(&command(&base, "pathlib2x", "import sys\nreturn [p for p in sys.path if isinstance(p, str)]")).unwrap();
  let paths: Vec<String> = paths.get_array("a").objects().iter().map(|p| p.string()).collect();
  assert!(!paths.iter().any(|p| p.is_empty() || p == "."), "{:?}", paths);
  let _ = std::fs::remove_dir_all(&root);
}
