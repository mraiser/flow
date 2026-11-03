//! Sharing code between JavaScript commands: a command imports another as a
//! module, `import other, { helper } from "flowlang:<lib>/<ctl>/<cmd>"` - the
//! counterpart of a Python command's `from lib.ctl import other`. Across
//! libraries (here in different library roots) and languages (JavaScript
//! importing TypeScript); an edit to an imported command reaches the
//! commands that import it; imports that name no JavaScript command are
//! errors at the import; stack frames name the imported command; check_js
//! type-checks across the import. Its own binary: a temporary store.
//!
//! Needs esbuild (as every command that imports does), unless
//! FLOWLANG_REQUIRE_ESBUILD is unset - then it checks only the error that
//! says esbuild is missing. The tsc assertions follow FLOWLANG_REQUIRE_TSC.
#![cfg(feature = "javascript_runtime")]

use flowlang::command::Command;
use flowlang::datastore::DataStore;
use ndata::dataobject::DataObject;
use serde_json::{json, Value};

fn have(tool: &str) -> bool {
  let var = format!("FLOWLANG_{}", tool.to_uppercase());
  let exe = std::env::var(&var).unwrap_or_else(|_| tool.to_string());
  std::process::Command::new(exe).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// Commands as newbound's dev.code writes them; ids are `<ctl>_<cmd>`.
struct Lib { name: &'static str, ctls: Vec<(String, Vec<String>)> }

impl Lib {
  fn new(name: &'static str, meta: &str) -> Lib {
    let store = DataStore::new();
    std::fs::create_dir_all(store.root.join(name)).unwrap();
    std::fs::write(store.root.join(name).join("meta.json"), meta).unwrap();
    Lib { name, ctls: Vec::new() }
  }

  fn put(&mut self, ctl: &str, cmd: &str, lang: &str, params: Value, imports: &str, body: &str) {
    let store = DataStore::new();
    let rec = |v: Value| DataObject::from_string(&json!({ "data": v }).to_string());
    let id = format!("{}_{}", ctl, cmd);
    store.set_data(self.name, &id, rec(json!({"type": lang, lang: format!("i_{}", id), "name": cmd})));
    store.set_data(self.name, &format!("i_{}", id), rec(json!({
      "type": lang, lang: body, "import": imports, "params": params, "returntype": "Any",
    })));
    match self.ctls.iter_mut().find(|(c, _)| c == ctl) {
      Some((_, cmds)) => { if !cmds.iter().any(|c| c == cmd) { cmds.push(cmd.to_string()); } }
      None => self.ctls.push((ctl.to_string(), vec![cmd.to_string()])),
    }
    let list: Vec<Value> = self.ctls.iter().map(|(c, _)| json!({"name": c, "id": format!("ctl_{}", c)})).collect();
    store.set_data(self.name, "controls", rec(json!({ "list": list })));
    for (c, cmds) in &self.ctls {
      let cmds: Vec<Value> = cmds.iter().map(|m| json!({"name": m, "id": format!("{}_{}", c, m)})).collect();
      store.set_data(self.name, &format!("ctl_{}", c), rec(json!({ "cmd": cmds })));
    }
  }
}

fn run(lib: &str, ctl: &str, cmd: &str, args: Value) -> Value {
  let r = Command::lookup(lib, ctl, cmd).execute(DataObject::from_string(&args.to_string())).unwrap();
  serde_json::from_str::<Value>(&r.to_string()).unwrap()["a"].clone()
}

fn value(lib: &str, ctl: &str, cmd: &str, args: Value) -> Value {
  let v = run(lib, ctl, cmd, args);
  assert!(v.get("status") != Some(&json!("err")), "{}:{}:{} failed: {}", lib, ctl, cmd, v["msg"].as_str().unwrap_or(""));
  v
}

fn error(lib: &str, ctl: &str, cmd: &str) -> String {
  let v = run(lib, ctl, cmd, json!({}));
  assert_eq!(v["status"], "err", "{}", v);
  v["msg"].as_str().unwrap().to_string()
}

fn check(lib: &str, ctl: &str, cmd: &str) -> Value {
  let r = flowlang::jscmd::check(lib, &format!("{}_{}", ctl, cmd));
  serde_json::from_str(&r.to_string()).unwrap()
}

#[test]
fn commands_import_commands() {
  let root = std::env::temp_dir().join(format!("flowlang_js_shared_{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&root);
  std::fs::create_dir_all(root.join("data")).unwrap();
  DataStore::init(root.join("data").to_str().unwrap());

  // sb lives in a library root of its own.
  let mut sb = Lib::new("sb", r#"{"root":"other"}"#);
  let s = json!([{"name": "s", "type": "String"}]);
  let x = json!([{"name": "x", "type": "String"}]);
  sb.put("tools", "text", "typescript", s.clone(),
    "export const shout = (s: string): string => s.toUpperCase();",
    "return 'text:' + s;");
  // A type error of its own, which is its own check's business.
  sb.put("tools", "sloppy", "typescript", json!([]), "", "const n: number = 'x';\nreturn n;");

  let mut sa = Lib::new("sa", "{}");
  let helpers_imports = "import { shout } from \"flowlang:sb/tools/text\";\n\
    export const exclaim = (s) => shout(s) + '!';\n\
    let counter = 0;\n\
    export const bump = () => ++counter;\n\
    export const boom = () => {\n  throw new Error('from the helper');\n};";
  sa.put("util", "helpers", "javascript", x.clone(), helpers_imports, "return exclaim(x);");
  sa.put("main", "uses", "javascript", json!([]),
    "import helpers, { exclaim, bump } from \"flowlang:sa/util/helpers\";\nimport text from 'flowlang:sb/tools/text';",
    "return { viaDefault: helpers('a'), exclaim: exclaim('b'), text: text('c'), bumps: [bump(), bump()] };");
  sa.put("main", "boomer", "javascript", json!([]), "import { boom } from \"flowlang:sa/util/helpers\";", "boom();\nreturn 1;");
  sa.put("main", "missing", "javascript", json!([]), "// shared code\nimport nothing from \"flowlang:sa/util/nosuch\";", "return nothing;");
  sa.put("main", "badname", "javascript", json!([]), "import x from \"flowlang:sa/helpers\";", "return x;");
  let mut rec = sa.ctls.clone();
  {
    // A Rust command, which can be called but not imported.
    let store = DataStore::new();
    let r = |v: Value| DataObject::from_string(&json!({ "data": v }).to_string());
    store.set_data("sa", "util_native", r(json!({"type": "rust", "rust": "i_util_native", "name": "native"})));
    store.set_data("sa", "i_util_native", r(json!({"params": [], "returntype": "Any"})));
    rec.iter_mut().find(|(c, _)| c == "util").unwrap().1.push("native".to_string());
    sa.ctls = rec;
  }
  sa.put("main", "native", "javascript", json!([]), "import n from \"flowlang:sa/util/native\";", "return n;");
  sa.put("main", "typed", "typescript", json!([]), "import text from \"flowlang:sb/tools/text\";", "return text(42);");
  sa.put("main", "importsloppy", "javascript", json!([]), "import sloppy from \"flowlang:sb/tools/sloppy\";", "return typeof sloppy;");

  if !have("esbuild") {
    assert!(std::env::var("FLOWLANG_REQUIRE_ESBUILD").is_err(), "esbuild is required (FLOWLANG_REQUIRE_ESBUILD)");
    assert!(error("sa", "main", "uses").contains("esbuild"));
    eprintln!("esbuild not installed: only the error that says so was checked");
    return;
  }

  // The default export is the command's function; named exports are its
  // module's; imports chain, across libraries and into TypeScript.
  assert_eq!(value("sa", "main", "uses", json!({})),
    json!({"viaDefault": "A!", "exclaim": "B!", "text": "text:c", "bumps": [1, 2]}));
  // Module state lives as long as the importing command's code.
  assert_eq!(value("sa", "main", "uses", json!({}))["bumps"], json!([3, 4]));
  // The imported command still runs as a command of its own.
  assert_eq!(value("sa", "util", "helpers", json!({"x": "own"})), json!("OWN!"));
  // Its generated file is where its library's root says.
  assert!(root.join("other/src/sb/tools/text.ts").exists());

  // Editing an imported command reaches the command importing it, two
  // levels up.
  sb.put("tools", "text", "typescript", s.clone(),
    "export const shout = (s: string): string => s.toUpperCase() + s.toUpperCase();",
    "return 'text:' + s;");
  assert_eq!(value("sa", "main", "uses", json!({}))["exclaim"], json!("BB!"));

  // A frame in an imported command is named for it.
  let msg = error("sa", "main", "boomer");
  assert!(msg.contains("from the helper") && msg.contains("flowlang:sa/util/helpers.js:"), "{}", msg);

  // Imports that name no JavaScript command fail at the import.
  let msg = error("sa", "main", "missing");
  assert!(msg.contains("cannot import \"flowlang:sa/util/nosuch\": there is no command sa:util:nosuch")
    && msg.contains("imports line 2"), "{}", msg);
  let msg = error("sa", "main", "badname");
  assert!(msg.contains("a command is imported as \"flowlang:<lib>/<ctl>/<cmd>\""), "{}", msg);
  let msg = error("sa", "main", "native");
  assert!(msg.contains("sa:util:native is a rust command; only JavaScript and TypeScript commands can be imported"), "{}", msg);

  if have("tsc") {
    let c = check("sa", "main", "uses");
    assert_eq!((c["tsc"].as_str(), c["clean"].as_bool()), (Some("ran"), Some(true)), "{}", c);
    // Types cross the import: text takes a string.
    let c = check("sa", "main", "typed");
    let p = &c["problems"][0];
    assert_eq!((p["tool"].as_str(), p["where"].as_str()), (Some("tsc"), Some("body line 1")), "{}", c);
    assert!(p["message"].as_str().unwrap().contains("not assignable to parameter of type 'string'"), "{}", c);
    // An imported command's own type error is not the importer's.
    let c = check("sa", "main", "importsloppy");
    assert_eq!((c["status"].as_str(), c["clean"].as_bool()), (Some("ok"), Some(true)), "{}", c);
    assert_eq!(check("sb", "tools", "sloppy")["clean"], json!(false));
  } else {
    assert!(std::env::var("FLOWLANG_REQUIRE_TSC").is_err(), "tsc is required (FLOWLANG_REQUIRE_TSC)");
  }

  let _ = std::fs::remove_dir_all(&root);
}
