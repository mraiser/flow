//! JavaScript commands as MCP tools, end to end: the real `flowmcp` binary
//! over stdin/stdout, against a temporary store. Its tools are listed with
//! their schemas, they run, their failures come back, and nothing a command
//! prints - console output, a timer firing after the call returned, an
//! on-demand esbuild run - reaches stdout, which is the JSON-RPC channel.
#![cfg(feature = "javascript_runtime")]

use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

fn write(path: &std::path::Path, s: &str) {
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(path, s).unwrap();
}

fn have_esbuild() -> bool {
  let exe = std::env::var("FLOWLANG_ESBUILD").unwrap_or_else(|_| "esbuild".to_string());
  Command::new(exe).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// A store with one library, `jm`, whose `tools` control holds `cmds`:
/// (name, lang, params, desc, body). Records are written through flowlang's
/// own DataStore so they are in its format.
fn store(root: &std::path::Path, cmds: &[(&str, &str, Value, &str, &str)]) {
  write(&root.join("config.properties"), "apps=jm\ndefault_app=jm\nhttp_port=0\nsecurity=on\n");
  write(&root.join("runtime/jm/app.properties"), "libraries=jm\n");
  write(&root.join("data/jm/meta.json"), r#"{"id":"jm","readers":[],"writers":[]}"#);
  flowlang::datastore::DataStore::init(root.join("data").to_str().unwrap());
  let store = flowlang::datastore::DataStore::new();
  let rec = |v: Value| ndata::dataobject::DataObject::from_string(&json!({ "data": v }).to_string());
  store.set_data("jm", "controls", rec(json!({"list": [{"name": "tools", "id": "ctl"}]})));
  let list: Vec<Value> = cmds.iter().map(|c| json!({"name": c.0, "id": format!("c_{}", c.0)})).collect();
  store.set_data("jm", "ctl", rec(json!({ "cmd": list })));
  for (name, lang, params, desc, body) in cmds {
    store.set_data("jm", &format!("c_{}", name), rec(json!({"type": lang, *lang: format!("i_{}", name), "name": name})));
    store.set_data("jm", &format!("i_{}", name), rec(json!({
      "type": lang, *lang: body, "params": params, "returntype": "Any", "desc": desc,
    })));
  }
}

/// Runs `flowmcp` in `root` with these requests; its stdout and stderr.
fn session(root: &std::path::Path, requests: &[Value]) -> (String, String) {
  let mut child = Command::new(env!("CARGO_BIN_EXE_flowmcp"))
    .current_dir(root)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("flowmcp did not start");
  {
    let mut stdin = child.stdin.take().unwrap();
    for r in requests { writeln!(stdin, "{}", r).unwrap(); }
  }
  let out = child.wait_with_output().unwrap();
  (String::from_utf8_lossy(&out.stdout).to_string(), String::from_utf8_lossy(&out.stderr).to_string())
}

fn call(id: i64, tool: &str, arguments: Value) -> Value {
  json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": tool, "arguments": arguments}})
}

/// Every stdout line is a JSON-RPC response; they are returned by id.
fn responses(stdout: &str, stderr: &str) -> std::collections::HashMap<i64, Value> {
  let mut by_id = std::collections::HashMap::new();
  for line in stdout.lines() {
    let v: Value = serde_json::from_str(line)
      .unwrap_or_else(|_| panic!("stdout carried a line that is not JSON-RPC: {:?}\n--- stderr:\n{}", line, stderr));
    assert_eq!(v["jsonrpc"], "2.0", "{}", line);
    by_id.insert(v["id"].as_i64().unwrap(), v);
  }
  by_id
}

fn text(r: &Value) -> String {
  r["result"]["content"][0]["text"].as_str().unwrap_or_default().to_string()
}

#[test]
fn javascript_commands_as_mcp_tools() {
  let root = std::env::temp_dir().join(format!("flowlang_js_mcp_{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&root);
  let who = json!([{"name": "who", "type": "String", "desc": "Who to greet."}]);
  let mut cmds = vec![
    ("hello", "javascript", who.clone(), "Greets someone.",
      "console.log('logging', who);\nconsole.info({ an: 'object' });\nconsole.error('to stderr');\nreturn 'hi ' + who"),
    ("fails", "javascript", json!([]), "Always fails.", "\nthrow new Error('nope')"),
    // A timer that prints after the call has returned, while the server
    // goes on to the next request.
    ("later", "javascript", json!([]), "Prints from a timer.",
      "setTimeout(() => console.log('late output'), 0);\nreturn 1"),
    ("waits", "javascript", json!([]), "Awaits a timer.",
      "await new Promise((r) => setTimeout(r, 20));\nconsole.log('after the wait');\nreturn { waited: true }"),
    ("hidden", "javascript", json!([]), "", "return 'no description, so not a tool'"),
  ];
  let esbuild = have_esbuild();
  if esbuild {
    // Bundled on its first call, inside the session: esbuild's own output
    // must not reach stdout either.
    cmds.push(("typed", "typescript", json!([{"name": "n", "type": "Integer"}]), "A TypeScript tool.",
      "const twice: number = n * 2;\nconsole.log('typed', twice);\nreturn twice"));
  } else {
    assert!(std::env::var("FLOWLANG_REQUIRE_ESBUILD").is_err(), "esbuild is required (FLOWLANG_REQUIRE_ESBUILD)");
    eprintln!("esbuild not installed: skipping the TypeScript tool");
  }
  store(&root, &cmds);

  let mut requests = vec![
    json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}),
    call(3, "jm-tools-hello", json!({"who": "there"})),
    call(4, "jm-tools-fails", json!({})),
    call(5, "jm-tools-later", json!({})),
    call(6, "jm-tools-waits", json!({})),
    call(7, "jm-tools-hello", json!({"who": "again"})),
  ];
  if esbuild { requests.push(call(8, "jm-tools-typed", json!({"n": 21}))); }
  let (stdout, stderr) = session(&root, &requests);
  let r = responses(&stdout, &stderr);
  assert_eq!(r.len(), requests.len(), "a response per request\n--- stdout:\n{}\n--- stderr:\n{}", stdout, stderr);

  // Listed with a schema from the declared parameters; one with no
  // description is not a tool.
  let tools = r[&2]["result"]["tools"].as_array().unwrap();
  let hello = tools.iter().find(|t| t["name"] == "jm-tools-hello").expect("jm-tools-hello listed");
  assert_eq!(hello["inputSchema"]["properties"]["who"]["type"], "string");
  assert_eq!(hello["inputSchema"]["required"], json!(["who"]));
  assert!(hello["description"].as_str().unwrap().starts_with("Greets someone."), "{}", hello);
  assert!(!tools.iter().any(|t| t["name"] == "jm-tools-hidden"));

  assert_eq!(text(&r[&3]), "hi there");
  assert_eq!(text(&r[&7]), "hi again");
  // The failure's report - message and place - reaches the client.
  let failed = text(&r[&4]);
  assert!(failed.contains("nope") && failed.contains("body line 2"), "{}", failed);
  assert_eq!(text(&r[&5]), "1");
  assert_eq!(serde_json::from_str::<Value>(&text(&r[&6])).unwrap(), json!({"waited": true}));
  if esbuild { assert_eq!(text(&r[&8]), "42"); }

  // What the commands printed went to stderr, labelled.
  for line in ["[js jm:tools:hello] logging there", r#"[js jm:tools:hello] {"an":"object"}"#,
               "[js jm:tools:hello] to stderr", "[js jm:tools:hello] logging again",
               "[js jm:tools:later] late output", "[js jm:tools:waits] after the wait"] {
    assert!(stderr.contains(line), "stderr lacks {:?}\n--- stderr:\n{}", line, stderr);
  }
  if esbuild { assert!(stderr.contains("[js jm:tools:typed] typed 42"), "{}", stderr); }

  let _ = std::fs::remove_dir_all(&root);
}
