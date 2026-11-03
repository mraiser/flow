//! JavaScript commands at build time. There is nothing to compile - the
//! engine (`jscmd`, `javascript_runtime`) runs a command's body straight from
//! the store - but flowb writes, for editors and for `check_js`'s type check:
//!
//! - `<lib root>/src/flowlang.d.ts`: the engine's globals (console, timers,
//!   `flowlang`) and `flowlang.api`, a typed tree of every command in every
//!   library in the store;
//! - `<lib root>/src/<lib>/<ctl>/<cmd>.js`: the command as a `// @ts-check`
//!   function whose JSDoc types come from its declared parameters and return
//!   type. Generated, like the Python sources: edit the command, not this
//!   file.
//!
//! Nothing here needs V8, so flowb writes these whether or not it was built
//! with `javascript_runtime`.

use std::fs::{create_dir_all, read_to_string};
use std::path::Path;

use crate::DataStore;

/// The language tags a command record may carry for JavaScript.
pub const JS_LANGS: [&str; 2] = ["js", "javascript"];

pub(crate) const FLOWLANG_DTS_BASE: &str = include_str!("flowlang_base.d.ts");

/// True when `await` appears in the body as a word of its own: the command
/// is then an async function. A mention in a string or comment makes the
/// command async too, which only means it answers through a Promise.
pub fn uses_await(body: &str) -> bool {
  let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '$';
  body.match_indices("await").any(|(i, _)| {
    let before = body[..i].chars().next_back();
    let after = body[i + 5..].chars().next();
    !before.map_or(false, is_word) && !after.map_or(false, is_word)
  })
}

const JS_RESERVED: [&str; 46] = [
  "await", "break", "case", "catch", "class", "const", "continue", "debugger", "default",
  "delete", "do", "else", "enum", "export", "extends", "false", "finally", "for", "function",
  "if", "implements", "import", "in", "instanceof", "interface", "let", "new", "null",
  "package", "private", "protected", "public", "return", "static", "super", "switch", "this",
  "throw", "true", "try", "typeof", "var", "void", "while", "with", "yield",
];

/// An ASCII JavaScript identifier that is not a reserved word: usable as a
/// parameter or function name.
pub fn is_identifier(s: &str) -> bool {
  is_property_name(s) && !JS_RESERVED.contains(&s)
}

/// An ASCII identifier, reserved words included: usable after a `.`.
fn is_property_name(s: &str) -> bool {
  let mut chars = s.chars();
  match chars.next() {
    Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '$' => {}
    _ => return false,
  }
  chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// The name a command, control or library has under `flowlang.api` (`-`
/// read as `_`, as the resolver does), if it has one.
fn api_name(name: &str) -> Option<String> {
  let n = name.replace('-', "_");
  if is_property_name(&n) { Some(n) } else { None }
}

/// The TypeScript type of a declared parameter, as a caller passes it and as
/// the body receives it.
pub fn ts_type(flow_type: &str) -> &'static str {
  match flow_type {
    "String" | "File" => "string",
    "Integer" | "Float" | "Double" => "number",
    "Boolean" => "boolean",
    "JSONObject" | "FLAT" => "Record<string, any>",
    "JSONArray" => "any[]",
    // ndata's JSON form of bytes: a space-separated hex string.
    "InputStream" => "string",
    _ => "any",
  }
}

/// A JavaScript command as the store holds it.
#[derive(Debug, Clone)]
pub struct JsSource {
  /// The command's name (the generated function's name when it is an
  /// identifier).
  pub name: String,
  /// Declared (name, type) pairs, in order.
  pub params: Vec<(String, String)>,
  pub returntype: String,
  pub body: String,
}

impl JsSource {
  /// Reads a JavaScript command's records: `None` if it is not one.
  pub fn read(store: &DataStore, lib: &str, id: &str) -> Option<JsSource> {
    if !store.exists(lib, id) { return None; }
    let cmd = store.get_data(lib, id).get_object("data");
    let impl_key = JS_LANGS.iter().copied().find(|k| cmd.has(k))?;
    let impl_id = cmd.get_string(impl_key);
    if !store.exists(lib, &impl_id) { return None; }
    let impl_def = store.get_data(lib, &impl_id).get_object("data");
    let body_key = JS_LANGS.iter().copied().find(|k| impl_def.has(k))?;
    let mut params = Vec::new();
    if impl_def.has("params") {
      for p in impl_def.get_array("params").objects() {
        let p = p.object();
        params.push((p.get_string("name"), p.get_string("type")));
      }
    }
    Some(JsSource {
      name: if impl_def.has("cmd") { impl_def.get_string("cmd") } else { cmd.get_string("name") },
      params,
      returntype: if impl_def.has("returntype") { impl_def.get_string("returntype") } else { "Any".to_string() },
      body: impl_def.get_string(body_key),
    })
  }

  pub fn is_async(&self) -> bool {
    uses_await(&self.body)
  }

  /// What the body may return, as JSDoc sees it.
  fn returns(&self) -> String {
    let t = match self.returntype.as_str() {
      "NULL" => "any",
      other => ts_type(other),
    };
    if self.is_async() { format!("Promise<{}>", t) } else { format!("{} | PromiseLike<{}>", t, t) }
  }
}

/// The lines before the body in a generated command file: body line N is
/// file line `COMMAND_HEADER_LINES + N`.
pub fn command_header_lines(src: &JsSource) -> usize {
  command_file(src, "flowlang.d.ts").lines().position(|l| l.ends_with(") {")).map(|i| i + 1).unwrap_or(0)
}

/// The generated `// @ts-check` file for a command; `dts` is the path of
/// flowlang.d.ts relative to it.
pub fn command_file(src: &JsSource, dts: &str) -> String {
  let name = if is_identifier(&src.name) { src.name.as_str() } else { "command" };
  let mut s = String::new();
  s.push_str("// @ts-check\n");
  s.push_str("// Generated by flowb from the store: edit the command, not this file.\n");
  s.push_str(&format!("/// <reference path=\"{}\" />\n", dts));
  s.push_str("/**\n");
  for (p, t) in &src.params {
    s.push_str(&format!(" * @param {{{}}} {}\n", ts_type(t), p));
  }
  s.push_str(&format!(" * @returns {{{}}}\n", src.returns()));
  s.push_str(" */\n");
  let names: Vec<&str> = src.params.iter().map(|(p, _)| p.as_str()).collect();
  s.push_str(&format!("{}function {}({}) {{\n", if src.is_async() { "async " } else { "" }, name, names.join(", ")));
  s.push_str(&src.body);
  if !src.body.ends_with('\n') { s.push('\n'); }
  s.push_str("}\n");
  s
}

/// flowb's build step for a JavaScript command: writes its generated file
/// and refreshes flowlang.d.ts. Returns false: there is no cargo artifact.
pub(crate) fn build_js_command(src_root: &Path, lib: &str, ctl: &str, cmd: &str, store: &DataStore) -> bool {
  let id = store.lookup_cmd_id(lib, ctl, cmd);
  let Some(src) = JsSource::read(store, lib, &id) else {
    println!("WARNING: {}:{}:{} has no JavaScript body", lib, ctl, cmd);
    return false;
  };
  let dir = src_root.join(lib).join(ctl);
  write_if_changed(&dir.join(format!("{}.js", cmd)), &command_file(&src, "../../flowlang.d.ts"));
  write_if_changed(&src_root.join("flowlang.d.ts"), &flowlang_dts(store));
  false
}

fn write_if_changed(path: &Path, content: &str) {
  if read_to_string(path).ok().as_deref() == Some(content) { return; }
  if let Some(parent) = path.parent() { let _ = create_dir_all(parent); }
  if let Err(e) = std::fs::write(path, content) {
    println!("WARNING: could not write {:?}: {}", path, e);
  }
}

/// flowlang.d.ts: the hand-written base plus `FlowlangApi`, a typed tree of
/// every command in every library in the store (any language). A name that
/// is not an identifier (after `-` reads as `_`) is left out of the tree -
/// flowlang.call still reaches it; a command with such a parameter is typed
/// `(...args: any[])`. A record that does not parse leaves out its control.
pub fn flowlang_dts(store: &DataStore) -> String {
  let mut libs: Vec<String> = std::fs::read_dir(&store.root)
    .map(|it| it.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect())
    .unwrap_or_default();
  libs.retain(|l| store.exists(l, "controls"));
  libs.sort();
  let mut tree = String::new();
  for lib in &libs {
    let Some(lib_name) = api_name(lib) else { continue };
    let controls = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      let list = store.get_data(lib, "controls").get_object("data").get_array("list");
      let mut v: Vec<(String, String)> = list.objects().into_iter()
        .map(|d| { let o = d.object(); (o.get_string("name"), o.get_string("id")) }).collect();
      v.sort();
      v
    })).unwrap_or_default();
    let mut lib_body = String::new();
    for (ctl, ctl_id) in controls {
      let Some(ctl_name) = api_name(&ctl) else { continue };
      let methods = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| api_methods(store, lib, &ctl, &ctl_id)))
        .unwrap_or_default();
      if methods.is_empty() { continue; }
      lib_body.push_str(&format!("    {}: {{\n{}    }};\n", ctl_name, methods));
    }
    if lib_body.is_empty() { continue; }
    tree.push_str(&format!("  {}: {{\n{}  }};\n", lib_name, lib_body));
  }
  let mut s = String::from(FLOWLANG_DTS_BASE);
  s.push_str("\n// ---- api: generated by flowb from the libraries in data/ ----\n\n");
  s.push_str("interface FlowlangApi {\n");
  s.push_str(&tree);
  s.push_str("}\n");
  s
}

/// One method signature per command of the control, sorted by name.
fn api_methods(store: &DataStore, lib: &str, ctl: &str, ctl_id: &str) -> String {
  let ctl_data = store.get_data(lib, ctl_id).get_object("data");
  if !ctl_data.has("cmd") { return String::new(); }
  let mut cmds: Vec<(String, String)> = ctl_data.get_array("cmd").objects().into_iter()
    .map(|d| { let o = d.object(); (o.get_string("name"), o.get_string("id")) }).collect();
  cmds.sort();
  let mut out = String::new();
  for (name, id) in cmds {
    let Some(method) = api_name(&name) else { continue };
    if !store.exists(lib, &id) { continue; }
    let cmd = store.get_data(lib, &id).get_object("data");
    if !cmd.has("type") { continue; }
    let typ = cmd.get_string("type");
    let impl_key = if cmd.has(&typ) { typ.clone() } else if let Some(k) = JS_LANGS.iter().find(|k| cmd.has(k)) { k.to_string() } else { continue };
    let impl_data = store.get_data(lib, &cmd.get_string(&impl_key)).get_object("data");
    let mut params = Vec::new();
    let mut plain = true;
    if impl_data.has("params") {
      for p in impl_data.get_array("params").objects() {
        let p = p.object();
        let pname = p.get_string("name");
        if !is_identifier(&pname) { plain = false; }
        params.push(format!("{}: {}", pname, ts_type(&p.get_string("type"))));
      }
    }
    let sig = if plain { params.join(", ") } else { "...args: any[]".to_string() };
    let returntype = if impl_data.has("returntype") { impl_data.get_string("returntype") } else { String::new() };
    let mut ret = match returntype.as_str() { "" | "NULL" => "any", t => ts_type(t) }.to_string();
    let is_js = JS_LANGS.contains(&typ.as_str());
    if is_js && JS_LANGS.iter().any(|k| impl_data.has(k) && uses_await(&impl_data.get_string(k))) {
      ret = format!("Promise<{}>", ret);
    }
    out.push_str(&format!("      /** {}:{}:{} ({}) */\n      {}({}): {};\n", lib, ctl, name, typ, method, sig, ret));
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  fn src(params: &[(&str, &str)], returntype: &str, body: &str) -> JsSource {
    JsSource {
      name: "cmd".into(),
      params: params.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
      returntype: returntype.into(),
      body: body.into(),
    }
  }

  #[test]
  fn await_is_found_as_a_word() {
    assert!(uses_await("return await x"));
    assert!(uses_await("await x;"));
    assert!(!uses_await("const awaited = 1; return awaited"));
    assert!(!uses_await("return x.awaitable"));
  }

  #[test]
  fn the_command_file_types_its_params_and_return() {
    let f = command_file(&src(&[("who", "String"), ("n", "Integer")], "JSONObject", "return {who, n};"), "../../flowlang.d.ts");
    assert!(f.starts_with("// @ts-check\n"), "{}", f);
    assert!(f.contains(" * @param {string} who\n * @param {number} n\n"), "{}", f);
    assert!(f.contains(" * @returns {Record<string, any> | PromiseLike<Record<string, any>>}\n"), "{}", f);
    assert!(f.contains("\nfunction cmd(who, n) {\nreturn {who, n};\n}\n"), "{}", f);
    let a = command_file(&src(&[], "String", "return await f()"), "x.d.ts");
    assert!(a.contains(" * @returns {Promise<string>}\n") && a.contains("\nasync function cmd() {\n"), "{}", a);
  }

  /// The generated declarations are valid TypeScript for this crate's own
  /// store (when tsc is installed; FLOWLANG_REQUIRE_TSC=1 requires it).
  #[test]
  fn flowlang_dts_compiles() {
    crate::builder::test_init();
    let dts = flowlang_dts(&DataStore::new());
    assert!(dts.contains("interface FlowlangApi {\n") && dts.contains("  testflow: {\n"), "{}", dts);
    assert!(dts.contains("      test_add("), "{}", dts);
    let dir = std::env::temp_dir().join(format!("flowlang_dts_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("flowlang.d.ts"), &dts).unwrap();
    std::fs::write(dir.join("tsconfig.json"), r#"{"compilerOptions":{"target":"es2022","lib":["es2022"],"types":[],"noEmit":true},"files":["flowlang.d.ts"]}"#).unwrap();
    let out = std::process::Command::new("tsc")
      .args(["-p", "tsconfig.json", "--pretty", "false"])
      .current_dir(&dir)
      .output();
    let _ = std::fs::remove_dir_all(&dir);
    match out {
      Err(_) if std::env::var("FLOWLANG_REQUIRE_TSC").is_err() => eprintln!("tsc not installed: not compiled"),
      Err(e) => panic!("tsc: {}", e),
      Ok(o) => assert!(o.status.success(), "{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)),
    }
  }

  #[test]
  fn header_lines_put_the_body_where_check_js_expects_it() {
    let s = src(&[("a", "Integer")], "Any", "line one\nline two");
    let f = command_file(&s, "flowlang.d.ts");
    let h = command_header_lines(&s);
    let lines: Vec<&str> = f.lines().collect();
    assert_eq!(lines[h], "line one");
    assert_eq!(lines[h + 1], "line two");
  }
}
