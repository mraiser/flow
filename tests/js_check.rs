//! check_js: a JavaScript command is checked without running it - a V8
//! syntax check always, and tsc's type check against flowlang.d.ts when
//! TypeScript is installed. Problems are reported at body lines. Its own
//! binary: a temporary store.
//!
//! With `FLOWLANG_REQUIRE_TSC=1` (CI), a missing tsc fails the test instead
//! of skipping the type-check assertions.
#![cfg(feature = "javascript_runtime")]

use flowlang::datastore::DataStore;
use flowlang::jscmd::{check, check_quick};
use ndata::dataobject::DataObject;

fn record(json: &str) -> DataObject { DataObject::from_string(json) }

fn problems(r: &DataObject) -> Vec<(String, String, String)> {
  r.get_array("problems").objects().into_iter().map(|p| {
    let p = p.object();
    (p.get_string("tool"), p.get_string("where"), p.get_string("message"))
  }).collect()
}

#[test]
fn checks_report_at_body_lines() {
  let root = std::env::temp_dir().join(format!("flowlang_js_check_{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&root);
  std::fs::create_dir_all(root.join("data").join("jc")).unwrap();
  DataStore::init(root.join("data").to_str().unwrap());
  let store = DataStore::new();
  let cmds = ["add", "syntax", "fetches", "wrong_return", "bad_call", "awaits"];
  let list: Vec<String> = cmds.iter().enumerate().map(|(i, n)| format!(r#"{{"name":"{}","id":"cmd{}"}}"#, n, i)).collect();
  store.set_data("jc", "controls", record(r#"{"data":{"list":[{"name":"tools","id":"ctl1"}]}}"#));
  store.set_data("jc", "ctl1", record(&format!(r#"{{"data":{{"cmd":[{}]}}}}"#, list.join(","))));
  let js = |i: usize, params: &str, returntype: &str, body: &str| {
    store.set_data("jc", &format!("cmd{}", i), record(&format!(r#"{{"data":{{"type":"javascript","javascript":"impl{}","name":"{}"}}}}"#, i, cmds[i])));
    let mut d = record(&format!(r#"{{"data":{{"type":"javascript","params":{},"returntype":"{}"}}}}"#, params, returntype));
    let mut data = d.get_object("data");
    data.put_string("javascript", body);
    d.put_object("data", data);
    store.set_data("jc", &format!("impl{}", i), d);
  };
  let ab = r#"[{"name":"a","type":"Integer"},{"name":"b","type":"Integer"}]"#;
  js(0, ab, "Integer", "return a + b");
  js(1, "[]", "Any", "let x = 1;\nreturn (");
  js(2, "[]", "Any", "const r = await fetch('https://example.com');\nreturn process.env.HOME");
  js(3, "[]", "String", "\n\nreturn 42");
  js(4, "[]", "Integer", "return flowlang.api.jc.tools.add(1)");
  js(5, "[]", "Integer", "const n = await flowlang.api.jc.tools.add(1, 2);\nreturn n");

  let require_tsc = std::env::var("FLOWLANG_REQUIRE_TSC").is_ok();
  let full = check("jc", "cmd0");
  assert_eq!(full.get_string("status"), "ok", "{}", full.to_string());
  assert_eq!(full.get_string("command"), "jc:tools:add");
  let tsc = full.get_string("tsc");
  assert!(full.get_boolean("clean"), "{}", full.to_string());
  if require_tsc { assert_eq!(tsc, "ran", "{}", full.to_string()); }

  // A syntax error: found by V8 whether or not tsc exists; tsc is skipped.
  for r in [check("jc", "cmd1"), check_quick("jc", "cmd1")] {
    assert!(!r.get_boolean("clean"));
    assert_eq!(r.get_string("tsc"), "skipped");
    let p = problems(&r);
    assert_eq!(p.len(), 1, "{:?}", p);
    assert_eq!(p[0].0, "syntax");
    assert!(p[0].2.contains("SyntaxError"), "{:?}", p);
  }

  // check_quick never runs tsc.
  let q = check_quick("jc", "cmd2");
  assert!(q.get_boolean("clean") && q.get_string("tsc") == "skipped", "{}", q.to_string());

  if tsc != "ran" {
    eprintln!("tsc not installed: skipping the type-check assertions");
    let _ = std::fs::remove_dir_all(&root);
    return;
  }

  // Names the engine does not provide.
  let p = problems(&check("jc", "cmd2"));
  assert!(p.iter().any(|(t, w, m)| t == "tsc" && w == "body line 2" && m.contains("process")), "{:?}", p);
  assert!(!p.iter().any(|(_, _, m)| m.contains("fetch")), "{:?}", p);
  // A return that does not match the declared type.
  let p = problems(&check("jc", "cmd3"));
  assert!(p.iter().any(|(t, w, m)| t == "tsc" && w == "body line 3" && m.contains("number")), "{:?}", p);
  // flowlang.api is typed from the store.
  let p = problems(&check("jc", "cmd4"));
  assert!(p.iter().any(|(t, w, m)| t == "tsc" && w == "body line 1" && m.contains("Expected 2 arguments")), "{:?}", p);
  // An awaiting body is async, and awaits fine.
  let r = check("jc", "cmd5");
  assert!(r.get_boolean("clean"), "{}", r.to_string());

  let _ = std::fs::remove_dir_all(&root);
}
