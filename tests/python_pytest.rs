//! `flow pytest [args]` runs pytest inside the embedded runtime: tests call
//! commands through flowlang.call / flowlang.api, import command modules and
//! use real ndata_py objects. Runs the real `flow` binary.

use std::process::Command;

const EMBEDDED: bool = cfg!(feature = "python_runtime");

fn flow_pytest(files: &[(&str, &str)]) -> (i32, String) {
  let dir = std::env::temp_dir().join(format!("flowlang_pytest_{}_{}", std::process::id(), files[0].0));
  let _ = std::fs::remove_dir_all(&dir);
  std::fs::create_dir_all(&dir).unwrap();
  for (name, src) in files { std::fs::write(dir.join(name), src).unwrap(); }
  let out = Command::new(env!("CARGO_BIN_EXE_flow"))
    .args(["pytest", "-q", "-p", "no:cacheprovider"])
    .arg(&dir)
    .current_dir(env!("CARGO_MANIFEST_DIR"))   // the data/ directory
    .output().unwrap();
  let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
  (out.status.code().unwrap_or(-1), text)
}

const PASSING: &str = r#"
import pytest
import flowlang
import ndata_py
from testflow.testflow import test_python as cmd_module

def test_api_call():
    assert flowlang.api.testflow.testflow.test_add(300, 120) == "300+120=420"

def test_command_function_directly():
    assert cmd_module.test_python("you") == "Hello you from python"

def test_real_wrappers():
    o = ndata_py.NDataObject()
    o["k"] = [1, 2]
    assert o == {"k": [1, 2]} and isinstance(o["k"], ndata_py.NDataArray)

def test_command_error():
    with pytest.raises(flowlang.CommandError, match="missing required parameter: a"):
        flowlang.call("testflow", "testflow", "test_python")

def test_output_is_captured_by_pytest(capsys):
    print("hi")
    assert capsys.readouterr().out == "hi\n"
"#;

const FAILING: &str = r#"
import flowlang

def test_wrong_sum():
    assert flowlang.api.testflow.testflow.test_add(1, 1) == "1+1=3"
"#;

#[test]
fn flow_pytest_runs_tests_in_the_embedded_runtime() {
  let (code, out) = flow_pytest(&[("test_passing.py", PASSING)]);
  if !EMBEDDED {
    assert_eq!(code, 2, "{}", out);
    assert!(out.contains("flow pytest needs the embedded Python runtime"), "{}", out);
    return;
  }
  if code == 4 && out.contains("pytest is not installed") {
    eprintln!("pytest not installed: skipping");
    return;
  }
  assert_eq!(code, 0, "{}", out);
  assert!(out.contains("5 passed"), "{}", out);

  let (code, out) = flow_pytest(&[("test_failing.py", FAILING)]);
  assert_eq!(code, 1, "{}", out);
  assert!(out.contains("1 failed") && out.contains("'1+1=2' == '1+1=3'"), "{}", out);
}
