//! flowlang::pycmd::check - a Python command's generated source checked
//! without running the command: compile, import, and mypy when installed.
//! Its own binary: it rewrites testflow's generated test_python.py (restored
//! afterwards, also on panic).

use flowlang::pycmd::{check, PyCmd};
use ndata::dataobject::DataObject;

const PY_ID: &str = "ovgzjp1822699e1d1w19";   // testflow:testflow:test_python
const RUST_ID: &str = "zkuwhn1802d57cb8ak1c"; // a non-Python command

struct Restore { path: String, original: String }
impl Drop for Restore {
  fn drop(&mut self) { let _ = std::fs::write(&self.path, &self.original); }
}

fn problems(r: &DataObject) -> Vec<(String, String, String, String)> {
  r.get_array("problems").objects().iter().map(|p| {
    let p = p.object();
    (p.get_string("tool"), p.get_string("severity"), p.get_string("where"), p.get_string("message"))
  }).collect()
}

fn mypy_installed() -> bool {
  ["python3", "python"].iter().any(|py| std::process::Command::new(py).args(["-c", "import mypy.api"])
    .output().map(|o| o.status.success()).unwrap_or(false))
}

#[test]
fn check_reports_problems_where_the_author_wrote_them() {
  flowlang::init("data");
  let path = PyCmd::get_path("testflow", PY_ID);
  let original = std::fs::read_to_string(&path).unwrap();
  let _restore = Restore { path: path.clone(), original: original.clone() };
  let with = |imports_extra: &str, body: &str| {
    let mut src = original.replace("import sys\nimport json\n", &format!("import sys\nimport json\n{}", imports_extra));
    src = src.replace("    # aaa\n    return 'Hello '+a+' from python'\n", body);
    std::fs::write(&path, &src).unwrap();
    check("testflow", PY_ID)
  };

  // The generated command as committed: clean.
  let r = check("testflow", PY_ID);
  assert_eq!(r.get_string("status"), "ok", "{}", r.to_string());
  assert_eq!(r.get_string("command"), "testflow:testflow:test_python");
  assert!(r.get_boolean("clean"), "{}", r.to_string());
  let mypy = mypy_installed();
  assert_eq!(r.get_string("mypy"), if mypy { "ran" } else { "not installed" }, "{}", r.to_string());

  // A syntax error in the body, at the line the author wrote it.
  let r = with("", "    x = 1\n    return (\n");
  assert!(!r.get_boolean("clean"));
  let p = problems(&r);
  assert_eq!(p.len(), 1, "{:?}", p);
  assert_eq!((p[0].0.as_str(), p[0].1.as_str(), p[0].2.as_str()), ("python", "error", "body line 2"), "{:?}", p);
  assert!(p[0].3.starts_with("SyntaxError: "), "{:?}", p);

  // A missing package: found by importing, reported at its imports line.
  let r = with("import no_such_package_xyz\n", "    return 'x'\n");
  let p = problems(&r);
  assert!(p.iter().any(|(t, s, w, m)| t == "import" && s == "error" && w == "imports line 3"
    && m.starts_with("ModuleNotFoundError: No module named 'no_such_package_xyz'")), "{:?}", p);

  // Module-level output does not corrupt the result.
  let r = with("print('noise at import')\n", "    return 'x'\n");
  assert!(r.get_boolean("clean"), "{}", r.to_string());

  // A type error, when mypy is installed: a is declared String.
  if mypy {
    let r = with("", "    return a + 1\n");
    let p = problems(&r);
    assert!(!r.get_boolean("clean"));
    assert!(p.iter().any(|(t, s, w, m)| t == "mypy" && s == "error" && w == "body line 1"
      && m.contains("Unsupported operand types for +")), "{:?}", p);
  } else {
    eprintln!("mypy not installed: skipping the type-error case");
  }

  // Not a Python command.
  let r = check("testflow", RUST_ID);
  assert_eq!(r.get_string("status"), "err", "{}", r.to_string());
  assert!(r.get_string("msg").contains("is not a Python command"), "{}", r.to_string());
}
