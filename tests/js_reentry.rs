//! JavaScript → Rust (or Python, or flow) → JavaScript. A JavaScript
//! command's `flowlang.call` to a command in another language runs that
//! command on a worker thread while the engine waits; when that command
//! calls a JavaScript command, the waiting engine runs it, in the same
//! isolate. Before, the inner call failed: the engine was busy with the
//! outer one. Its own binary: a temporary store, a Rust command registered
//! by hand, and a watchdog stop at the end.
#![cfg(feature = "javascript_runtime")]

use flowlang::command::Command;
use flowlang::datastore::DataStore;
use flowlang::rustcmd::RustCmd;
use ndata::dataobject::DataObject;

/// The Rust command: runs `jr:tools:<cmd>` with `args` and returns its
/// envelope as its own, so an inner failure is this command's failure.
fn bridge(a: DataObject) -> DataObject {
  let cmd = a.get_string("cmd");
  let args = if a.has("args") { a.get_object("args") } else { DataObject::new() };
  match std::panic::catch_unwind(|| Command::lookup("jr", "tools", &cmd).execute(args)) {
    Ok(Ok(r)) => r,
    Ok(Err(e)) => {
      let mut err = DataObject::new();
      err.put_string("status", "err");
      err.put_string("msg", &format!("{:?}", e));
      let mut o = DataObject::new();
      o.put_object("a", err);
      o
    }
    Err(_) => panic!("bridge: {} panicked", cmd),
  }
}

fn store(root: &std::path::Path) {
  std::fs::create_dir_all(root.join("data").join("jr")).unwrap();
  DataStore::init(root.join("data").to_str().unwrap());
  let mut globals = DataStore::globals();
  if !globals.has("RUST_COMMANDS") { globals.put_object("RUST_COMMANDS", DataObject::new()); }
  let mut cmds = globals.get_object("RUST_COMMANDS");
  cmds.put_object("rimpl", RustCmd::detail("rimpl".to_string(), bridge, "".to_string()));

  let store = DataStore::new();
  let rec = |json: &str| DataObject::from_string(json);
  store.set_data("jr", "controls", rec(r#"{"data":{"list":[{"name":"tools","id":"ctl1"}]}}"#));
  let names = ["bridge", "outer", "inner", "seen", "thrower", "via_thrower", "awaiter", "via_awaiter",
    "mutate", "relay", "deep", "after_await", "spin", "via_spin", "fine", "pybridge", "via_python", "pydeep", "jsdeep"];
  let list: Vec<String> = names.iter().map(|n| format!(r#"{{"name":"{}","id":"c_{}"}}"#, n, n)).collect();
  store.set_data("jr", "ctl1", rec(&format!(r#"{{"data":{{"cmd":[{}]}}}}"#, list.join(","))));

  store.set_data("jr", "c_bridge", rec(r#"{"data":{"type":"rust","rust":"rimpl","name":"bridge"}}"#));
  store.set_data("jr", "rimpl", rec(r#"{"data":{"params":[{"name":"cmd","type":"String"},{"name":"args","type":"JSONObject"}],"returntype":"Any"}}"#));

  let js = |name: &str, params: &str, body: &str| {
    store.set_data("jr", &format!("c_{}", name), rec(&format!(r#"{{"data":{{"type":"javascript","javascript":"i_{}","name":"{}"}}}}"#, name, name)));
    let mut d = rec(&format!(r#"{{"data":{{"type":"javascript","params":{},"returntype":"Any"}}}}"#, params));
    let mut data = d.get_object("data");
    data.put_string("javascript", body);
    d.put_object("data", data);
    store.set_data("jr", &format!("i_{}", name), d);
  };
  let none = "[]";
  let x = r#"[{"name":"x","type":"Integer"}]"#;
  let o = r#"[{"name":"o","type":"JSONObject"}]"#;
  let n = r#"[{"name":"n","type":"Integer"}]"#;
  js("inner", x, "globalThis.innerRan = (globalThis.innerRan || 0) + 1;\nreturn { doubled: x * 2 }");
  js("outer", none, "globalThis.innerRan = 0;\nconst r = flowlang.api.jr.tools.bridge('inner', { x: 21 });\nreturn { r, innerRan: globalThis.innerRan }");
  js("thrower", none, "\nthrow new Error('inner boom')");
  js("via_thrower", none, "try { flowlang.api.jr.tools.bridge('thrower', {}); return 'no error' }\ncatch (e) { return { name: e.name, msg: e.message, isCmd: e instanceof flowlang.CommandError } }");
  js("awaiter", none, "await null;\nreturn 1");
  js("via_awaiter", none, "try { flowlang.api.jr.tools.bridge('awaiter', {}); return 'no error' }\ncatch (e) { return e.message }");
  js("mutate", o, "o.count = (o.count || 0) + 1;\nreturn null");
  js("relay", o, "flowlang.api.jr.tools.bridge('mutate', { o });\nflowlang.api.jr.tools.bridge('mutate', { o });\nreturn o.count");
  // JS -> Rust -> JS -> Rust -> ... n levels down.
  js("deep", n, "if (n === 0) return 'bottom';\nreturn n + '>' + flowlang.api.jr.tools.bridge('deep', { n: n - 1 })");
  js("after_await", none, "await null;\nconst r = flowlang.api.jr.tools.bridge('inner', { x: 5 });\nreturn r.doubled");
  js("spin", none, "for (;;) {}");
  js("via_spin", none, "flowlang.api.jr.tools.bridge('spin', {});\nreturn 'unreachable'");
  js("fine", none, "return 'fine'");

  // The same through Python: JavaScript -> Python -> JavaScript.
  store.set_data("jr", "c_pybridge", rec(r#"{"data":{"type":"python","python":"i_pybridge","name":"pybridge"}}"#));
  store.set_data("jr", "i_pybridge", rec(r#"{"data":{"params":[{"name":"x","type":"Integer"}],"returntype":"Any"}}"#));
  js("via_python", none, "globalThis.innerRan = 0;\nconst r = flowlang.api.jr.tools.pybridge(8);\nreturn { r, innerRan: globalThis.innerRan }");
  store.set_data("jr", "c_pydeep", rec(r#"{"data":{"type":"python","python":"i_pydeep","name":"pydeep"}}"#));
  store.set_data("jr", "i_pydeep", rec(r#"{"data":{"params":[{"name":"n","type":"Integer"}],"returntype":"Any"}}"#));
  js("jsdeep", n, "if (n === 0) return 'bottom';\nreturn 'js' + n + '>' + flowlang.api.jr.tools.pydeep(n - 1)");
  #[cfg(feature = "python_runtime")]
  {
    let path = flowlang::pycmd::PyCmd::get_path("jr", "c_pydeep");
    std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap()).unwrap();
    std::fs::write(&path, "import flowlang\n\ndef execute(args):\n    return pydeep(args['n'])\n\ndef pydeep(n: \"int\"):\n    if n == 0:\n        return 'bottom'\n    return 'py%d>%s' % (n, flowlang.call('jr', 'tools', 'jsdeep', n=n - 1))\n").unwrap();
  }
  #[cfg(feature = "python_runtime")]
  {
    let path = flowlang::pycmd::PyCmd::get_path("jr", "c_pybridge");
    std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap()).unwrap();
    std::fs::write(&path, "import flowlang\n\ndef execute(args):\n    return pybridge(args['x'])\n\ndef pybridge(x: \"int\"):\n    return {'py': flowlang.call('jr', 'tools', 'inner', x=x)}\n").unwrap();
  }
}

fn run(cmd: &str, args: DataObject) -> serde_json::Value {
  let r = Command::lookup("jr", "tools", cmd).execute(args).unwrap();
  serde_json::from_str::<serde_json::Value>(&r.to_string()).unwrap()["a"].clone()
}

fn v(s: &str) -> serde_json::Value { serde_json::from_str(s).unwrap() }

#[test]
fn javascript_through_rust_to_javascript() {
  let root = std::env::temp_dir().join(format!("flowlang_js_reentry_{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&root);
  store(&root);

  // The inner command runs in the outer one's isolate: it sees, and sets,
  // the same globals.
  assert_eq!(run("outer", DataObject::new()), v(r#"{"r":{"doubled":42},"innerRan":1}"#));

  // An inner exception comes back through the Rust command as its failure,
  // with the inner command's own place in it.
  let r = run("via_thrower", DataObject::new());
  assert_eq!(r["name"], "CommandError", "{}", r);
  assert_eq!(r["isCmd"], true);
  let msg = r["msg"].as_str().unwrap();
  assert!(msg.contains("inner boom") && msg.contains("(body line 2)") && msg.contains("flowlang:jr/tools/thrower.js:2:"), "{}", msg);

  // A live object, from Rust through JavaScript and Rust to JavaScript and back.
  let o = DataObject::from_string(r#"{"name":"x"}"#);
  let mut args = DataObject::new();
  args.put_object("o", o.clone());
  assert_eq!(run("relay", args), v("2"));
  assert_eq!(o.get_int("count"), 2);

  // Any depth.
  let mut args = DataObject::new();
  args.put_int("n", 6);
  assert_eq!(run("deep", args), v(r#""6>5>4>3>2>1>bottom""#));

  #[cfg(feature = "python_runtime")]
  assert_eq!(run("via_python", DataObject::new()), v(r#"{"r":{"py":{"doubled":16}},"innerRan":1}"#));
  #[cfg(feature = "python_runtime")]
  {
    let mut args = DataObject::new();
    args.put_int("n", 5);
    assert_eq!(run("jsdeep", args), v(r#""js5>py4>js3>py2>js1>bottom""#));
  }

  // From the async part of an async command.
  assert_eq!(run("after_await", DataObject::new()), v("10"));

  // An inner command that awaits cannot finish while the Rust command waits.
  let r = run("via_awaiter", DataObject::new());
  assert!(r.as_str().unwrap().contains("jr:tools:awaiter awaits, so it cannot run inside a call"), "{}", r);

  // An inner command that never returns is stopped at the outer call's time
  // limit; the Rust command between them is let go, and the engine carries on.
  flowlang::jscmd::set_default_timeout(Some(std::time::Duration::from_millis(300)));
  let r = Command::lookup("jr", "tools", "via_spin").execute(DataObject::new()).unwrap();
  let msg = r.get_object("a").get_string("msg");
  assert!(msg.contains("jr:tools:via_spin timed out after 300 ms"), "{}", msg);
  flowlang::jscmd::set_default_timeout(None);
  assert_eq!(run("fine", DataObject::new()), v(r#""fine""#));
  assert_eq!(run("outer", DataObject::new()), v(r#"{"r":{"doubled":42},"innerRan":1}"#));

  let _ = std::fs::remove_dir_all(&root);
}
