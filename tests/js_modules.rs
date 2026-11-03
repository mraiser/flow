//! JavaScript commands that import modules, and TypeScript commands: both run
//! from an esbuild bundle. Packages come from node_modules next to data/ -
//! here, small packages the test writes itself (CommonJS, and ESM with an
//! exports map and types), so no network is needed. Its own binary: a
//! temporary store.
//!
//! Needs esbuild (`$FLOWLANG_ESBUILD`, `node_modules/.bin/esbuild` next to
//! data/, or PATH); without it the test only checks the error that says so,
//! unless `FLOWLANG_REQUIRE_ESBUILD=1` (CI) makes that a failure. The tsc
//! assertions likewise follow `FLOWLANG_REQUIRE_TSC`.
#![cfg(feature = "javascript_runtime")]

use flowlang::command::Command;
use flowlang::datastore::DataStore;
use flowlang::jscmd::{check, check_quick};
use ndata::dataobject::DataObject;
use std::path::Path;

fn record(json: &str) -> DataObject { DataObject::from_string(json) }

fn write(path: &Path, text: &str) {
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(path, text).unwrap();
}

struct Store { lib: &'static str, names: Vec<String> }

impl Store {
  /// A command record as newbound's dev.code writes it: type and keys named
  /// for the language, imports under "import".
  fn add(&mut self, lang: &str, name: &str, params: &str, returntype: &str, imports: &str, body: &str) -> String {
    let store = DataStore::new();
    let i = self.names.len();
    self.names.push(name.to_string());
    let id = format!("cmd{}", i);
    store.set_data(self.lib, &id, record(&format!(r#"{{"data":{{"type":"{l}","{l}":"impl{i}","name":"{n}"}}}}"#, l = lang, i = i, n = name)));
    let mut d = record(&format!(r#"{{"data":{{"type":"{}","params":{},"returntype":"{}"}}}}"#, lang, params, returntype));
    let mut data = d.get_object("data");
    data.put_string(lang, body);
    data.put_string("import", imports);
    d.put_object("data", data);
    store.set_data(self.lib, &format!("impl{}", i), d);
    let list: Vec<String> = self.names.iter().enumerate().map(|(i, n)| format!(r#"{{"name":"{}","id":"cmd{}"}}"#, n, i)).collect();
    store.set_data(self.lib, "ctl1", record(&format!(r#"{{"data":{{"cmd":[{}]}}}}"#, list.join(","))));
    id
  }

  fn edit(&self, id: &str, body: &str) {
    let store = DataStore::new();
    let impl_id = id.replace("cmd", "impl");
    let mut d = store.get_data(self.lib, &impl_id);
    let mut data = d.get_object("data");
    let lang = data.get_string("type");
    data.put_string(&lang, body);
    d.put_object("data", data);
    store.set_data(self.lib, &impl_id, d);
  }
}

fn call(id: &str, args: &str) -> DataObject {
  Command::new("jm", id).execute(DataObject::from_string(args)).unwrap()
}

fn value(id: &str, args: &str) -> String {
  let r = call(id, args);
  if let Ok(ndata::data::Data::DObject(_)) = r.try_get_property("a") {
    let a = r.get_object("a");
    if a.has("status") && a.get_string("status") == "err" { panic!("{} failed: {}", id, a.get_string("msg")); }
  }
  r.to_string()
}

fn error(id: &str, args: &str) -> String {
  let r = call(id, args);
  let a = r.get_object("a");
  assert_eq!(a.get_string("status"), "err", "{}", r.to_string());
  a.get_string("msg")
}

fn problems(r: &DataObject) -> Vec<(String, String, String)> {
  r.get_array("problems").objects().into_iter().map(|p| {
    let p = p.object();
    (p.get_string("tool"), p.get_string("where"), p.get_string("message"))
  }).collect()
}

fn have(tool: &str) -> bool {
  let var = format!("FLOWLANG_{}", tool.to_uppercase());
  let exe = std::env::var(&var).unwrap_or_else(|_| tool.to_string());
  std::process::Command::new(exe).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

#[test]
fn modules_and_typescript() {
  let root = std::env::temp_dir().join(format!("flowlang_js_modules_{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&root);
  std::fs::create_dir_all(root.join("data").join("jm")).unwrap();
  let nm = root.join("node_modules");
  // CommonJS, with a function that throws, for the npm: frame.
  write(&nm.join("greet/package.json"), r#"{"name":"greet","version":"1.0.0","main":"index.js"}"#);
  write(&nm.join("greet/index.js"), "module.exports = function greet(who) { return 'hello ' + who; };\nmodule.exports.fail = function fail() {\n  throw new Error('from the package');\n};\n");
  // ESM with an exports map and types.
  write(&nm.join("esm-pkg/package.json"), r#"{"name":"esm-pkg","version":"1.0.0","type":"module","exports":{".":{"types":"./index.d.ts","import":"./index.js"}}}"#);
  write(&nm.join("esm-pkg/index.js"), "export const twice = (n) => n * 2;\nexport default 'esm default';\n");
  write(&nm.join("esm-pkg/index.d.ts"), "export declare const twice: (n: number) => number;\ndeclare const d: string;\nexport default d;\n");
  write(&root.join("package-lock.json"), "{}");

  DataStore::init(root.join("data").to_str().unwrap());
  let store = DataStore::new();
  store.set_data("jm", "controls", record(r#"{"data":{"list":[{"name":"tools","id":"ctl1"}]}}"#));
  let mut s = Store { lib: "jm", names: vec![] };
  let who = r#"[{"name":"who","type":"String"}]"#;
  let n = r#"[{"name":"n","type":"Integer"}]"#;

  let cjs = s.add("javascript", "cjs", who, "String", "import greet from \"greet\";", "return greet(who)");
  let esm = s.add("javascript", "esm", n, "JSONObject", "import label, { twice } from \"esm-pkg\";", "return { label, twice: twice(n) }");
  let throws = s.add("javascript", "throws", "[]", "Any", "import greet from \"greet\";\nimport { twice } from \"esm-pkg\";", "const x = twice(1);\nthrow new Error('mine ' + x)");
  let deep = s.add("javascript", "deep", "[]", "Any", "import { fail } from \"greet\";", "return fail()");
  let missing = s.add("javascript", "missing", "[]", "Any", "import nope from \"no-such-package\";", "return nope");
  let node = s.add("javascript", "node", "[]", "Any", "import fs from \"fs\";", "return typeof fs");
  let ts = s.add("typescript", "ts", n, "String", "import { twice } from \"esm-pkg\";", "const doubled: number = twice(n);\nreturn `${doubled}`;");
  let ts_plain = s.add("typescript", "ts_plain", n, "Integer", "", "const t: number = n + 1;\nreturn t;");
  let ts_bad = s.add("typescript", "ts_bad", n, "String", "", "const s: string = n;\nreturn s;");
  let ts_syntax = s.add("typescript", "ts_syntax", "[]", "Any", "", "let x: = 1;\nreturn x;");
  let caller = s.add("javascript", "caller", "[]", "Any", "", "return [flowlang.api.jm.tools.cjs('js'), flowlang.api.jm.tools.ts_plain(41)]");

  if !have("esbuild") {
    assert!(std::env::var("FLOWLANG_REQUIRE_ESBUILD").is_err(), "esbuild is required (FLOWLANG_REQUIRE_ESBUILD)");
    let e = error(&cjs, r#"{"who":"x"}"#);
    assert!(e.contains("esbuild is not installed"), "{}", e);
    eprintln!("esbuild not installed: skipping the bundle assertions");
    let _ = std::fs::remove_dir_all(&root);
    return;
  }

  // CommonJS and ESM packages, default and named imports.
  assert_eq!(value(&cjs, r#"{"who":"there"}"#), r#"{"a":"hello there"}"#);
  let r: serde_json::Value = serde_json::from_str(&value(&esm, r#"{"n":21}"#)).unwrap();
  assert_eq!(r, serde_json::json!({"a": {"label": "esm default", "twice": 42}}));

  // Errors point into the author's code - through the source map - and into
  // packages as npm:<path>.
  let e = error(&throws, "{}");
  assert!(e.contains("mine 2") && e.contains("(body line 2)") && e.contains("flowlang:jm/tools/throws.js:"), "{}", e);
  let e = error(&deep, "{}");
  assert!(e.contains("from the package") && e.contains("npm:greet/index.js:3:"), "{}", e);
  assert!(e.contains("(body line 1)") || e.contains("flowlang:jm/tools/deep.js"), "{}", e);

  // Resolution problems are found before anything runs, at imports lines.
  let e = error(&missing, "{}");
  assert!(e.contains("Could not resolve \"no-such-package\" (imports line 1)") && e.contains("npm install no-such-package"), "{}", e);
  let e = error(&node, "{}");
  assert!(e.contains("Could not resolve \"fs\" (imports line 1)") && e.contains("Node's built-in modules") && !e.contains("--platform"), "{}", e);

  // An edited body is rebundled; the bundle lives next to the generated file.
  s.edit(&cjs, "return greet(who).toUpperCase()");
  assert_eq!(value(&cjs, r#"{"who":"there"}"#), r#"{"a":"HELLO THERE"}"#);
  let gen = root.join("cmd/src/jm/tools");
  assert!(gen.join("cjs.js").exists() && gen.join("cjs.bundle.js").exists());

  // TypeScript, with and without imports; called from JavaScript too.
  assert_eq!(value(&ts, r#"{"n":21}"#), r#"{"a":"42"}"#);
  assert_eq!(value(&ts_plain, r#"{"n":1}"#), r#"{"a":2}"#);
  assert!(gen.join("ts.ts").exists() && gen.join("ts_plain.bundle.js").exists());
  let r: serde_json::Value = serde_json::from_str(&value(&caller, "{}")).unwrap();
  assert_eq!(r, serde_json::json!({"a": ["HELLO JS", 42]}));
  // esbuild does not type-check: a type error still runs (tsc finds it).
  assert_eq!(value(&ts_bad, r#"{"n":3}"#), r#"{"a":3}"#);
  let e = error(&ts_syntax, "{}");
  assert!(e.contains("(body line 1)"), "{}", e);

  // check / check_quick.
  let q = check_quick("jm", &missing);
  let p = problems(&q);
  assert!(!q.get_boolean("clean") && p.iter().any(|(t, w, m)| t == "esbuild" && w == "imports line 1" && m.contains("no-such-package")), "{:?}", p);
  let q = check_quick("jm", &ts_syntax);
  assert!(problems(&q).iter().any(|(t, w, _)| t == "esbuild" && w == "body line 1"), "{}", q.to_string());
  assert!(check_quick("jm", &esm).get_boolean("clean"));

  if !have("tsc") {
    assert!(std::env::var("FLOWLANG_REQUIRE_TSC").is_err(), "tsc is required (FLOWLANG_REQUIRE_TSC)");
    eprintln!("tsc not installed: skipping the type-check assertions");
    let _ = std::fs::remove_dir_all(&root);
    return;
  }
  for id in [&cjs, &esm, &ts, &ts_plain] {
    let r = check("jm", id);
    assert!(r.get_boolean("clean") && r.get_string("tsc") == "ran", "{}: {}", id, r.to_string());
  }
  let p = problems(&check("jm", &ts_bad));
  assert!(p.iter().any(|(t, w, m)| t == "tsc" && w == "body line 1" && m.contains("not assignable to type 'string'")), "{:?}", p);

  let _ = std::fs::remove_dir_all(&root);
}
