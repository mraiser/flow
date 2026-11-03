//! Deploying JavaScript/TypeScript commands that run from esbuild bundles:
//! `flowb BUNDLES` builds them where esbuild and the packages are, and a
//! deployment that ships `data/` plus the `.bundle.js` files - no
//! node_modules, no lockfile, no esbuild - runs them. A bundle built from
//! other code is an error there, not a silent run of old code; a lockfile
//! whose content is unchanged (only touched) does not force a rebuild.
//!
//! Runs the real `flowb` and `flow` binaries in temporary roots. Needs
//! esbuild for the build side; without it the test is skipped, unless
//! `FLOWLANG_REQUIRE_ESBUILD=1` (CI).
#![cfg(feature = "javascript_runtime")]

use flowlang::datastore::DataStore;
use ndata::dataobject::DataObject;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn record(json: &str) -> DataObject { DataObject::from_string(json) }

fn write(path: &Path, text: &str) {
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(path, text).unwrap();
}

fn copy_dir(from: &Path, to: &Path) {
  std::fs::create_dir_all(to).unwrap();
  for e in std::fs::read_dir(from).unwrap().flatten() {
    let p = e.path();
    if p.is_dir() { copy_dir(&p, &to.join(e.file_name())); } else { std::fs::copy(&p, to.join(e.file_name())).unwrap(); }
  }
}

/// Every `*.bundle.js` under `from`, copied to the same relative path under
/// `to` - what packaging ships.
fn copy_bundles(from: &Path, to: &Path, rel: &Path) {
  for e in std::fs::read_dir(from.join(rel)).unwrap().flatten() {
    let r = rel.join(e.file_name());
    if e.path().is_dir() { copy_bundles(from, to, &r); }
    else if r.to_string_lossy().ends_with(".bundle.js") {
      std::fs::create_dir_all(to.join(&r).parent().unwrap()).unwrap();
      std::fs::copy(e.path(), to.join(&r)).unwrap();
    }
  }
}

/// Runs a binary in `dir`; `no_esbuild` makes esbuild explicitly absent.
fn run(bin: &str, dir: &Path, args: &[&str], stdin: &str, no_esbuild: bool) -> (i32, String) {
  let mut c = Command::new(bin);
  c.args(args).current_dir(dir).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
  if no_esbuild { c.env("FLOWLANG_ESBUILD", dir.join("no-such-esbuild")); }
  let mut child = c.spawn().unwrap();
  child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
  let out = child.wait_with_output().unwrap();
  (out.status.code().unwrap_or(-1), format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
}

fn have_esbuild() -> bool {
  let exe = std::env::var("FLOWLANG_ESBUILD").unwrap_or_else(|_| "esbuild".to_string());
  Command::new(exe).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

fn set_body(store: &DataStore, impl_id: &str, lang: &str, body: &str) {
  let mut d = store.get_data("dm", impl_id);
  let mut data = d.get_object("data");
  data.put_string(lang, body);
  d.put_object("data", data);
  store.set_data("dm", impl_id, d);
}

#[test]
fn shipped_bundles_run_without_esbuild() {
  if !have_esbuild() {
    assert!(std::env::var("FLOWLANG_REQUIRE_ESBUILD").is_err(), "esbuild is required (FLOWLANG_REQUIRE_ESBUILD)");
    eprintln!("esbuild not installed: skipping");
    return;
  }
  let flowb = env!("CARGO_BIN_EXE_flowb");
  let flow = env!("CARGO_BIN_EXE_flow");
  let base = std::env::temp_dir().join(format!("flowlang_js_deploy_{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&base);
  let dev: PathBuf = base.join("dev");

  // The development root: a package, its lockfile, and two commands that run
  // from bundles - JavaScript with an import, and TypeScript.
  write(&dev.join("node_modules/greet/package.json"), r#"{"name":"greet","version":"1.0.0","main":"index.js"}"#);
  write(&dev.join("node_modules/greet/index.js"), "module.exports = (who) => 'hello ' + who;\n");
  write(&dev.join("package-lock.json"), r#"{"name":"dev","lockfileVersion":3,"packages":{"node_modules/greet":{"version":"1.0.0"}}}"#);
  std::fs::create_dir_all(dev.join("data/dm")).unwrap();
  DataStore::init(dev.join("data").to_str().unwrap());
  let store = DataStore::new();
  store.set_data("dm", "controls", record(r#"{"data":{"list":[{"name":"tools","id":"ctl1"}]}}"#));
  store.set_data("dm", "ctl1", record(r#"{"data":{"cmd":[{"name":"hi","id":"cmd1"},{"name":"twice","id":"cmd2"},{"name":"plain","id":"cmd3"}]}}"#));
  store.set_data("dm", "cmd1", record(r#"{"data":{"type":"javascript","javascript":"impl1","name":"hi"}}"#));
  store.set_data("dm", "impl1", record(r#"{"data":{"type":"javascript","params":[{"name":"who","type":"String"}],"returntype":"String","import":"import greet from \"greet\";"}}"#));
  set_body(&store, "impl1", "javascript", "return greet(who)");
  store.set_data("dm", "cmd2", record(r#"{"data":{"type":"typescript","typescript":"impl2","name":"twice"}}"#));
  store.set_data("dm", "impl2", record(r#"{"data":{"type":"typescript","params":[{"name":"n","type":"Integer"}],"returntype":"Integer","import":""}}"#));
  set_body(&store, "impl2", "typescript", "const d: number = n * 2;\nreturn d;");
  // Plain JavaScript runs from the store: no bundle.
  store.set_data("dm", "cmd3", record(r#"{"data":{"type":"javascript","javascript":"impl3","name":"plain"}}"#));
  store.set_data("dm", "impl3", record(r#"{"data":{"type":"javascript","params":[],"returntype":"String","import":""}}"#));
  set_body(&store, "impl3", "javascript", "return 'plain'");

  let (code, out) = run(flowb, &dev, &["BUNDLES"], "", false);
  assert_eq!(code, 0, "{}", out);
  assert!(out.contains("bundled dm:tools:hi") && out.contains("bundled dm:tools:twice") && out.contains("2 bundled, 0 failed"), "{}", out);
  let gen = dev.join("cmd/src/dm/tools");
  let hi_bundle = gen.join("hi.bundle.js");
  assert!(hi_bundle.exists() && gen.join("twice.bundle.js").exists() && !gen.join("plain.bundle.js").exists());

  // A touched lockfile (same content) does not rebuild; new content does.
  let before = std::fs::read_to_string(&hi_bundle).unwrap();
  let lock = std::fs::read_to_string(dev.join("package-lock.json")).unwrap();
  std::thread::sleep(std::time::Duration::from_millis(1100));
  std::fs::write(dev.join("package-lock.json"), &lock).unwrap();
  let (code, out) = run(flowb, &dev, &["BUNDLES"], "", false);
  assert_eq!(code, 0, "{}", out);
  assert_eq!(std::fs::read_to_string(&hi_bundle).unwrap(), before, "a touched lockfile rebuilt the bundle");
  std::fs::write(dev.join("package-lock.json"), lock.replace("1.0.0", "1.0.1")).unwrap();
  let (code, out) = run(flowb, &dev, &["BUNDLES"], "", false);
  assert_eq!(code, 0, "{}", out);
  let rebuilt = std::fs::read_to_string(&hi_bundle).unwrap();
  assert_ne!(rebuilt.lines().next(), before.lines().next(), "new lockfile content did not rebuild the bundle");

  // The deployment: data/ and the bundles - no node_modules, no lockfile,
  // no esbuild.
  let deployed = base.join("deployed");
  copy_dir(&dev.join("data"), &deployed.join("data"));
  copy_bundles(&dev, &deployed, Path::new("cmd"));
  assert!(!deployed.join("node_modules").exists() && !deployed.join("package-lock.json").exists());
  let (code, out) = run(flow, &deployed, &["dm", "tools", "hi"], r#"{"who":"deployed"}"#, true);
  assert!(code == 0 && out.contains(r#"{"a":"hello deployed"}"#), "{} {}", code, out);
  let (code, out) = run(flow, &deployed, &["dm", "tools", "twice"], r#"{"n":21}"#, true);
  assert!(code == 0 && out.contains(r#"{"a":42}"#), "{} {}", code, out);
  let (_, out) = run(flow, &deployed, &["dm", "tools", "plain"], "{}", true);
  assert!(out.contains(r#"{"a":"plain"}"#), "{}", out);

  // A deployment whose command changed after its bundle was built: an error
  // that says what to do - never the old code.
  set_body(&store, "impl1", "javascript", "return greet(who).toUpperCase()");
  let stale = base.join("stale");
  copy_dir(&dev.join("data"), &stale.join("data"));
  copy_bundles(&deployed, &stale, Path::new("cmd"));
  let (_, out) = run(flow, &stale, &["dm", "tools", "hi"], r#"{"who":"x"}"#, true);
  assert!(out.contains("no bundle for its current code here and no esbuild") && !out.contains("hello x"), "{}", out);
  // flowb BUNDLES fails the same way, so a package build stops.
  let (code, out) = run(flowb, &stale, &["BUNDLES"], "", true);
  assert!(code == 1 && out.contains("FAILED dm:tools:hi") && out.contains("1 bundled, 1 failed"), "{} {}", code, out);

  let _ = std::fs::remove_dir_all(&base);
}
