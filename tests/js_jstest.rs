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

// ---- test files that import (bundled with esbuild) ----

fn have_esbuild() -> bool {
  let exe = std::env::var("FLOWLANG_ESBUILD").unwrap_or_else(|_| "esbuild".to_string());
  Command::new(exe).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// Whether to run the esbuild cases; without esbuild, checks that saying so
/// is the failure (unless FLOWLANG_REQUIRE_ESBUILD makes it a test failure).
fn esbuild_or_skip() -> bool {
  if have_esbuild() { return true; }
  assert!(std::env::var("FLOWLANG_REQUIRE_ESBUILD").is_err(), "esbuild is required (FLOWLANG_REQUIRE_ESBUILD)");
  let (code, out) = jstest(&[("i.test.js", "import x from './x.js';\ntest('t', () => {});\n")]);
  assert!(code == 1 && out.contains("there is no esbuild"), "{}", out);
  eprintln!("esbuild not installed: only the error that says so was checked");
  false
}

const IMPORTING: &str = r#"// Helpers and a package, before the tests.
/* a block
   comment */
import greet, { fail } from "greet";
import {
  double,
  explode,
} from "./fixtures.js";

const ready = await Promise.resolve("top-level await");

test("uses its imports", () => {
  assert.equal(greet(double(2)), "hello 4");
  assert.equal(ready, "top-level await");
  assert.equal(flowlang.api.testflow.testflow.test_add(1, 2), "1+2=3");
});

test("a failure keeps the file's line", () => {
  assert.equal(double(1), 3);
});

test("a frame in a helper is named for it", () => {
  explode();
});
"#;

#[test]
fn a_test_file_imports_packages_and_files_beside_it() {
  if !esbuild_or_skip() { return; }
  let dir = std::env::temp_dir().join(format!("flowlang_jstest_imports_{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&dir);
  std::fs::create_dir_all(dir.join("node_modules/greet")).unwrap();
  std::fs::write(dir.join("node_modules/greet/package.json"), r#"{"name":"greet","version":"1.0.0","main":"index.js"}"#).unwrap();
  std::fs::write(dir.join("node_modules/greet/index.js"), "module.exports = (who) => 'hello ' + who;\nmodule.exports.fail = () => { throw new Error('pkg'); };\n").unwrap();
  std::fs::write(dir.join("fixtures.js"), "export const double = (n) => n * 2;\nexport function explode() {\n  throw new Error('from the fixture');\n}\n").unwrap();
  std::fs::write(dir.join("imp.test.js"), IMPORTING).unwrap();
  std::fs::write(dir.join("missing.test.js"), "// nothing here\n\nimport nope from \"no-such-package\";\ntest('t', () => {});\n").unwrap();
  std::fs::write(dir.join("late.test.js"), "import { double } from './fixtures.js';\ntest('t', () => {});\nimport greet from 'greet';\n").unwrap();
  let out = Command::new(env!("CARGO_BIN_EXE_flow")).arg("jstest").arg(&dir).current_dir(env!("CARGO_MANIFEST_DIR")).output().unwrap();
  let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
  let _ = std::fs::remove_dir_all(&dir);
  assert_eq!(out.status.code(), Some(1), "{}", text);
  assert!(text.contains("✓ uses its imports"), "{}", text);
  // Line 19 of the file, though its imports moved.
  assert!(text.contains("✗ a failure keeps the file's line") && text.contains("imp.test.js:19:10)") && text.contains("imp.test.js:23:3)"), "{}", text);
  assert!(text.contains("from the fixture") && text.contains("fixtures.js:3:9)"), "{}", text);
  // An import that does not resolve, at its own line.
  assert!(text.contains(r#"Could not resolve "no-such-package""#) && text.contains("(line 3)"), "{}", text);
  assert!(text.contains("imports must come first in a test file") && text.contains("(line 3)"), "{}", text);
  assert!(text.contains("1 passed, 4 failed"), "{}", text);
  assert!(!text.contains("flowlang:jstest"), "{}", text);
}

/// A test file imports a command's helpers - `flowlang:<lib>/<ctl>/<cmd>` -
/// to test them directly. A store of its own, with one JavaScript command.
#[test]
fn a_test_file_imports_a_command() {
  if !esbuild_or_skip() { return; }
  let root = std::env::temp_dir().join(format!("flowlang_jstest_cmd_{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&root);
  let w = |p: &str, s: &str| {
    let p = root.join(p);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, s).unwrap();
  };
  let rec = |v: serde_json::Value| serde_json::json!({ "data": v }).to_string();
  // Records as files, in the store's layout (an id under 16 characters is
  // padded with _ for its directories).
  let put = |id: &str, v: serde_json::Value| {
    let padded = format!("{:_<16}", id);
    let path = format!("data/jt/{}/{}/{}/{}/{}", &padded[0..4], &padded[4..8], &padded[8..12], &padded[12..16], id);
    w(&path, &rec(v));
  };
  w("data/jt/meta.json", "{}");
  put("controls", serde_json::json!({"list": [{"name": "text", "id": "ctl"}]}));
  put("ctl", serde_json::json!({"cmd": [{"name": "fmt", "id": "c_fmt"}]}));
  put("c_fmt", serde_json::json!({"type": "javascript", "javascript": "i_fmt", "name": "fmt"}));
  put("i_fmt", serde_json::json!({"type": "javascript", "javascript": "return slug(s);", "params": [{"name": "s", "type": "String"}],
    "returntype": "String", "import": "export const slug = (s) => s.toLowerCase().replace(/[^a-z0-9]+/g, '-');"}));
  w("t/fmt.test.js", "import fmt, { slug } from \"flowlang:jt/text/fmt\";\n\ntest('the helper', () => assert.equal(slug('Hello World'), 'hello-world'));\ntest('the function', () => assert.equal(fmt('A B'), 'a-b'));\ntest('the command', () => assert.equal(flowlang.api.jt.text.fmt('C D'), 'c-d'));\n");
  w("t/bad.test.js", "import x from \"flowlang:jt/text/nope\";\ntest('t', () => {});\n");
  w("t/typed.test.ts", "import fmt, { slug } from \"flowlang:jt/text/fmt\";\n\ntest('typed', () => { const s: string = slug('X Y'); assert.equal(s, 'x-y'); assert.equal(fmt('Q R') as string, 'q-r'); });\n");
  let out = Command::new(env!("CARGO_BIN_EXE_flow")).arg("jstest").arg("t").current_dir(&root).output().unwrap();
  let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
  let _ = std::fs::remove_dir_all(&root);
  assert!(text.contains("✓ the helper") && text.contains("✓ the function") && text.contains("✓ the command"), "{}", text);
  assert!(text.contains("cannot import \"flowlang:jt/text/nope\": there is no command jt:text:nope (line 1)"), "{}", text);
  assert!(text.contains("✓ typed"), "{}", text);
  assert!(text.contains("4 passed, 1 failed"), "{}", text);
}

// ---- TypeScript test files (*.test.ts), always bundled with esbuild ----

const TYPED: &str = r#"interface Point { x: number; y: number }
enum Axis { X = "x", Y = "y" }
const norm = <T extends Point>(p: T): number => Math.abs(p.x) + Math.abs(p.y);

test("types, generics and enums", () => {
  const p = { x: -2, y: 3 } as Point;
  assert.equal(norm(p), 5);
  assert.equal(Axis.Y, "y");
  assert.equal(flowlang.api.testflow.testflow.test_add(2, 2) as string, "2+2=4");
});

test("a failure keeps the file's line", () => {
  const n: number = norm({ x: 1, y: 1 });
  assert.equal(n, 3);
});
"#;

const TYPED_IMPORTS: &str = r#"import { scale, type Scaled } from "./geometry.ts";

const s: Scaled = scale(2, 21);

test("imports a TypeScript file beside it", () => {
  assert.equal(s.value, 42);
});
"#;

#[test]
fn typescript_test_files_run() {
  if !esbuild_or_skip() { return; }
  let dir = std::env::temp_dir().join(format!("flowlang_jstest_ts_{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&dir);
  std::fs::create_dir_all(&dir).unwrap();
  std::fs::write(dir.join("typed.test.ts"), TYPED).unwrap();
  std::fs::write(dir.join("imports.test.ts"), TYPED_IMPORTS).unwrap();
  std::fs::write(dir.join("geometry.ts"), "export interface Scaled { value: number }\nexport const scale = (k: number, n: number): Scaled => ({ value: k * n });\n").unwrap();
  std::fs::write(dir.join("broken.test.ts"), "test('t', () => {});\n\nconst x: = 1;\n").unwrap();
  // Found beside the JavaScript ones.
  std::fs::write(dir.join("plain.test.js"), "test('plain', () => assert.ok(true));\n").unwrap();
  let out = Command::new(env!("CARGO_BIN_EXE_flow")).arg("jstest").arg(&dir).current_dir(env!("CARGO_MANIFEST_DIR")).output().unwrap();
  let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
  let _ = std::fs::remove_dir_all(&dir);
  assert_eq!(out.status.code(), Some(1), "{}", text);
  assert!(text.contains("✓ types, generics and enums") && text.contains("✓ imports a TypeScript file beside it") && text.contains("✓ plain"), "{}", text);
  assert!(text.contains("✗ a failure keeps the file's line") && text.contains("expected 3, got 2") && text.contains("typed.test.ts:14:10)"), "{}", text);
  assert!(text.contains("broken.test.ts") && text.contains("the file failed") && text.contains("(line 3)"), "{}", text);
  assert!(text.contains("3 passed, 2 failed"), "{}", text);
  assert!(!text.contains("flowlang:jstest"), "{}", text);
}
