//! Reference counts across the JavaScript boundary, as python_refcounts.rs
//! checks them for Python: once a command's handles are garbage (and V8 and
//! ndata have collected), every ndata object it created is freed. A separate
//! test binary, so no other test's objects are live meanwhile; V8's gc() is
//! exposed through FLOWLANG_JS_V8_FLAGS.
#![cfg(feature = "javascript_runtime")]

use flowlang::datastore::DataStore;
use flowlang::jscmd::{run, JsCall};
use ndata::dataobject::DataObject;

fn call(label: &str, body: &str) -> JsCall {
  JsCall {
    label: format!("rc:ctl:{}", label),
    params: vec![],
    body: body.to_string(),
    returntype: "Any".to_string(),
    args: DataObject::new(),
    timeout: None,
    module: None,
  }
}

/// Live ndata objects (as python_refcounts.rs counts them). Every case
/// keeps objects inside its arrays, so a leaked array leaks objects too.
fn live() -> usize {
  // A full V8 collection finalizes unreachable handles; their ndata
  // references are then released by ndata's GC.
  run(&call("collect", "gc(); gc(); return null;")).unwrap();
  for _ in 0..3 { DataStore::gc(); }
  ndata::dataobject::oheap().lock().keys().len()
}

#[test]
fn no_object_outlives_its_last_handle() {
  std::env::set_var("FLOWLANG_JS_V8_FLAGS", "--expose-gc");
  let root = std::env::temp_dir().join(format!("flowlang_js_refcounts_{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&root);
  std::fs::create_dir_all(root.join("data")).unwrap();
  DataStore::init(root.join("data").to_str().unwrap());
  let cases = [
    ("objects", "const o = ndata.object();\nfor (let i = 0; i < 200; i++) o['k' + (i % 7)] = { n: i, list: [i] };\nreturn null;"),
    ("arrays", "const a = ndata.array();\nfor (let i = 0; i < 200; i++) { a.push({ n: i }); if (a.length > 3) a.splice(0, 1); }\na.length = 0;\nreturn null;"),
    ("reads", "const o = ndata.object();\no.x = { y: { z: [1, 2, 3] } };\nlet s = 0;\nfor (let i = 0; i < 200; i++) s += o.x.y.z.length + JSON.stringify(o).length;\nreturn s > 0;"),
    ("returned", "return Array.from({ length: 200 }, (_, i) => ({ n: i, inner: [i] }));"),
  ];
  for (name, body) in cases {
    let c = call(name, body);
    run(&c).unwrap();  // warm up: install the command
    let before = live();
    let env = run(&c).unwrap();
    drop(env);
    let after = live();
    assert_eq!(after, before, "{}: {} ndata objects leaked", name, after as isize - before as isize);
  }
  let _ = std::fs::remove_dir_all(&root);
}
