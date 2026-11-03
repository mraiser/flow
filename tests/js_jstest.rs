//! `flow jstest`: JavaScript test files run in the engine commands run in,
//! with test() and assert as locals and flowlang.call reaching any command.
#![cfg(feature = "javascript_runtime")]

use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static SEQ: AtomicUsize = AtomicUsize::new(0);

fn jstest(files: &[(&str, &str)]) -> (i32, String) {
  let dir = std::env::temp_dir().join(format!("flowlang_jstest_{}_{}", std::process::id(), SEQ.fetch_add(1, Ordering::SeqCst)));
  let _ = std::fs::remove_dir_all(&dir);
  std::fs::create_dir_all(dir.join("node_modules")).unwrap();
  // Never collected: inside node_modules.
  std::fs::write(dir.join("node_modules").join("dep.test.js"), "test('dep', () => { throw new Error('collected') })").unwrap();
  for (name, src) in files {
    std::fs::write(dir.join(name), src).unwrap();
  }
  let out = Command::new(env!("CARGO_BIN_EXE_flow"))
    .arg("jstest").arg(&dir)
    .current_dir(env!("CARGO_MANIFEST_DIR"))
    .output()
    .unwrap();
  let _ = std::fs::remove_dir_all(&dir);
  let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
  (out.status.code().unwrap_or(-1), text)
}

const PASSING: &str = r#"
test("calls a flow command", () => {
  assert.equal(flowlang.api.testflow.testflow.test_add(300, 120), "300+120=420");
});

test("awaits", async () => {
  await new Promise((r) => setTimeout(r, 5));
  assert.deepEqual({ a: [1, { b: 2 }] }, { a: [1, { b: 2 }] });
  await assert.rejects(() => Promise.reject(new TypeError("nope")), TypeError);
  assert.throws(() => flowlang.call("testflow", "testflow", "no_such"), ReferenceError);
});

test.skip("later", () => {});
"#;

const FAILING: &str = r#"
test("wrong sum", () => {
  assert.equal(flowlang.api.testflow.testflow.test_add(1, 1), "1+1=3");
});
"#;

#[test]
fn passing_tests_exit_0() {
  let (code, out) = jstest(&[("ok.test.js", PASSING)]);
  assert_eq!(code, 0, "{}", out);
  assert!(out.contains("✓ calls a flow command") && out.contains("✓ awaits") && out.contains("- later (skipped)"), "{}", out);
  assert!(out.contains("2 passed, 0 failed, 1 skipped"), "{}", out);
  assert!(!out.contains("dep"), "{}", out);
}

#[test]
fn a_failure_names_the_file_line_and_exits_1() {
  let (code, out) = jstest(&[("bad.test.js", FAILING), ("ok.test.js", PASSING)]);
  assert_eq!(code, 1, "{}", out);
  assert!(out.contains("✗ wrong sum") && out.contains(r#"AssertionError: expected "1+1=3", got "1+1=2""#), "{}", out);
  assert!(out.contains("bad.test.js:3:10)"), "{}", out);
  assert!(!out.contains("flowlang:jstest"), "{}", out);
}

#[test]
fn a_file_that_does_not_load_fails() {
  let (code, out) = jstest(&[("syntax.test.js", "test('x', () => {});\nconst = 1;\n")]);
  assert_eq!(code, 1, "{}", out);
  assert!(out.contains("the file failed") && out.contains("SyntaxError") && out.contains("(line 2)"), "{}", out);
}

#[test]
fn nothing_to_run_exits_2() {
  let (code, out) = jstest(&[]);
  assert_eq!(code, 2, "{}", out);
}
