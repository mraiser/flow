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
    // Bytes cross as a Uint8Array.
    "InputStream" => "Uint8Array",
    _ => "any",
  }
}

/// A JavaScript or TypeScript command as the store holds it.
#[derive(Debug, Clone)]
pub struct JsSource {
  /// The command's name (the generated function's name when it is an
  /// identifier).
  pub name: String,
  /// The record's language: `javascript`, `js`, `typescript` or `ts`.
  pub lang: String,
  /// Declared (name, type) pairs, in order.
  pub params: Vec<(String, String)>,
  pub returntype: String,
  /// The command's imports (ES `import` statements), possibly empty.
  pub imports: String,
  pub body: String,
}

impl JsSource {
  /// Reads a JavaScript or TypeScript command's records: `None` if it is
  /// not one.
  pub fn read(store: &DataStore, lib: &str, id: &str) -> Option<JsSource> {
    if !store.exists(lib, id) { return None; }
    let cmd = store.get_data(lib, id).get_object("data");
    let typ = if cmd.has("type") { cmd.get_string("type") } else { String::new() };
    let impl_key = record_key(&cmd, &typ)?;
    let impl_id = cmd.get_string(impl_key);
    if !store.exists(lib, &impl_id) { return None; }
    let impl_def = store.get_data(lib, &impl_id).get_object("data");
    let body_key = record_key(&impl_def, &typ)?;
    let mut params = Vec::new();
    if impl_def.has("params") {
      for p in impl_def.get_array("params").objects() {
        let p = p.object();
        params.push((p.get_string("name"), p.get_string("type")));
      }
    }
    Some(JsSource {
      name: if impl_def.has("cmd") { impl_def.get_string("cmd") } else { cmd.get_string("name") },
      lang: if is_lang(&typ) { typ } else { body_key.to_string() },
      params,
      returntype: if impl_def.has("returntype") { impl_def.get_string("returntype") } else { "Any".to_string() },
      imports: if impl_def.has("import") { impl_def.get_string("import") } else { String::new() },
      body: impl_def.get_string(body_key),
    })
  }

  pub fn is_async(&self) -> bool {
    uses_await(&self.body)
  }

  pub fn is_typescript(&self) -> bool {
    TS_LANGS.contains(&self.lang.as_str())
  }

  /// True when the command runs from an esbuild bundle: it imports modules,
  /// or it is TypeScript.
  pub fn needs_bundle(&self) -> bool {
    self.is_typescript() || !self.imports.trim().is_empty()
  }

  /// The generated file's extension.
  pub fn extension(&self) -> &'static str {
    if self.is_typescript() { "ts" } else { "js" }
  }

  /// What the body may return.
  fn returns(&self) -> String {
    let t = match self.returntype.as_str() {
      "NULL" => "any",
      other => ts_type(other),
    };
    if self.is_async() { format!("Promise<{}>", t) } else { format!("{} | PromiseLike<{}>", t, t) }
  }

  fn import_lines(&self) -> usize {
    if self.imports.trim().is_empty() { 0 } else { self.imports.lines().count() }
  }
}

/// The TypeScript language tags.
pub const TS_LANGS: [&str; 2] = ["typescript", "ts"];

/// True for a command `type` the JavaScript engine runs: JavaScript or
/// TypeScript.
pub fn is_lang(t: &str) -> bool {
  JS_LANGS.contains(&t) || TS_LANGS.contains(&t)
}

/// The key of a command or implementation record that names (or holds) the
/// code: the record's own type first, then any JavaScript/TypeScript key.
fn record_key(record: &ndata::dataobject::DataObject, typ: &str) -> Option<&'static str> {
  let all = [JS_LANGS[0], JS_LANGS[1], TS_LANGS[0], TS_LANGS[1]];
  if let Some(k) = all.iter().copied().find(|k| *k == typ) {
    if record.has(k) { return Some(k); }
  }
  all.iter().copied().find(|k| record.has(k))
}

/// Where the parts of a generated command file are, in file lines (1-based).
#[derive(Debug, Clone, Copy)]
pub struct Layout {
  pub imports_first: usize,
  pub imports_lines: usize,
  pub body_first: usize,
  pub body_lines: usize,
}

impl Layout {
  /// The author's own name for a line of the generated file.
  pub fn describe(&self, file_line: usize) -> String {
    if self.imports_lines > 0 && file_line >= self.imports_first && file_line < self.imports_first + self.imports_lines {
      format!("imports line {}", file_line - self.imports_first + 1)
    } else if file_line >= self.body_first && file_line < self.body_first + self.body_lines {
      format!("body line {}", file_line - self.body_first + 1)
    } else if file_line >= self.body_first + self.body_lines {
      "the end of the body".to_string()
    } else {
      format!("line {} of the generated file", file_line)
    }
  }
}

/// The layout of `command_file(src, ..)`.
pub fn layout(src: &JsSource) -> Layout {
  let file = command_file(src, "flowlang.d.ts");
  let lines: Vec<&str> = file.lines().collect();
  let imports_first = lines.iter().position(|l| l.starts_with("/// <reference")).map(|i| i + 2).unwrap_or(1);
  let header = lines.iter().position(|l| l.starts_with("export default ") && l.ends_with('{')).map(|i| i + 1).unwrap_or(0);
  Layout {
    imports_first,
    imports_lines: src.import_lines(),
    body_first: header + 1,
    body_lines: src.body.lines().count().max(1),
  }
}

/// The lines before the body in a generated command file: body line N is
/// file line `command_header_lines + N`.
pub fn command_header_lines(src: &JsSource) -> usize {
  layout(src).body_first - 1
}

/// The generated file for a command: `// @ts-check` JavaScript with JSDoc
/// types from the declared parameters and return type, or TypeScript with
/// the same types written out. The command's imports come first, then the
/// command as the module's default export. `dts` is the path of
/// flowlang.d.ts relative to the file. esbuild bundles this file, tsc checks
/// it, and editors open it.
pub fn command_file(src: &JsSource, dts: &str) -> String {
  let name = if is_identifier(&src.name) { src.name.as_str() } else { "command" };
  let mut s = String::new();
  if !src.is_typescript() { s.push_str("// @ts-check\n"); }
  s.push_str("// Generated by flowb from the store: edit the command, not this file.\n");
  s.push_str(&format!("/// <reference path=\"{}\" />\n", dts));
  if src.import_lines() > 0 {
    s.push_str(src.imports.trim_end_matches('\n'));
    s.push('\n');
  }
  let asynchronous = if src.is_async() { "async " } else { "" };
  if src.is_typescript() {
    let params: Vec<String> = src.params.iter().map(|(p, t)| format!("{}: {}", p, ts_type(t))).collect();
    s.push_str(&format!("export default {}function {}({}): {} {{\n", asynchronous, name, params.join(", "), src.returns()));
  } else {
    s.push_str("/**\n");
    for (p, t) in &src.params {
      s.push_str(&format!(" * @param {{{}}} {}\n", ts_type(t), p));
    }
    s.push_str(&format!(" * @returns {{{}}}\n", src.returns()));
    s.push_str(" */\n");
    let names: Vec<&str> = src.params.iter().map(|(p, _)| p.as_str()).collect();
    s.push_str(&format!("export default {}function {}({}) {{\n", asynchronous, name, names.join(", ")));
  }
  s.push_str(&src.body);
  if !src.body.ends_with('\n') { s.push('\n'); }
  s.push_str("}\n");
  s
}

/// flowb's build step for a JavaScript or TypeScript command: writes its
/// generated file, refreshes flowlang.d.ts, and - for a command that needs
/// one, when esbuild is installed - its bundle. Returns false: there is no
/// cargo artifact.
pub(crate) fn build_js_command(src_root: &Path, lib: &str, ctl: &str, cmd: &str, store: &DataStore) -> bool {
  let id = store.lookup_cmd_id(lib, ctl, cmd);
  let Some(src) = JsSource::read(store, lib, &id) else {
    println!("WARNING: {}:{}:{} has no JavaScript body", lib, ctl, cmd);
    return false;
  };
  let dir = src_root.join(lib).join(ctl);
  write_if_changed(&dir.join(format!("{}.{}", cmd, src.extension())), &command_file(&src, "../../flowlang.d.ts"));
  write_if_changed(&src_root.join("flowlang.d.ts"), &flowlang_dts(store));
  if src.needs_bundle() {
    match bundle(store, src_root, lib, ctl, cmd, &src) {
      Ok(_) => {}
      Err(BundleError::NoEsbuild) => println!("NOTE: {}:{}:{} needs esbuild to run (npm install esbuild next to data/)", lib, ctl, cmd),
      Err(e) => println!("WARNING: {}:{}:{} does not bundle: {}", lib, ctl, cmd, e.text(&layout(&src))),
    }
  }
  false
}

fn write_if_changed(path: &Path, content: &str) {
  if read_to_string(path).ok().as_deref() == Some(content) { return; }
  if let Some(parent) = path.parent() { let _ = create_dir_all(parent); }
  if let Err(e) = std::fs::write(path, content) {
    println!("WARNING: could not write {:?}: {}", path, e);
  }
}

// ---- bundles --------------------------------------------------------------------------------

/// A command bundled by esbuild: one script that defines
/// `__flowlang_module` (the generated file's exports) with every import
/// inlined, and its source map, whose sources are named
/// `flowlang:<lib>/<ctl>/<cmd>.<ext>` (the generated file) and
/// `npm:<package path>` (code from node_modules).
#[derive(Debug, Clone)]
pub struct Bundle {
  /// Identifies the bundle's inputs (`<source key>-<packages key>`):
  /// changes when the command, its imports or the installed packages
  /// change.
  pub key: String,
  pub code: String,
  pub map: Vec<u8>,
  /// The generated file's name in the source map and in stack traces.
  pub source_name: String,
}

/// An esbuild diagnostic: where in the generated file (if known), what, and
/// esbuild's notes - rewritten where its advice (bundler flags) does not
/// apply to a command.
#[derive(Debug, Clone)]
pub struct Diagnostic {
  pub line: Option<usize>,
  pub column: Option<usize>,
  pub message: String,
  pub notes: Vec<String>,
}

impl Diagnostic {
  /// The message, the note lines indented beneath it.
  pub fn full(&self, place: Option<&str>) -> String {
    let mut s = self.message.clone();
    if let Some(p) = place { s.push_str(&format!(" ({})", p)); }
    for n in &self.notes { s.push_str("\n  "); s.push_str(n); }
    s
  }
}

/// esbuild's note, or what to say instead.
fn note(text: &str, message: &str) -> Option<String> {
  let package = message.strip_prefix("Could not resolve \"").and_then(|m| m.split('"').next()).unwrap_or("");
  if text.contains("is built into node") {
    Some(format!("\"{}\" is one of Node's built-in modules, which the engine does not have: use fetch and the web APIs, or a command in Rust or Python for files and processes.", package))
  } else if text.starts_with("You can mark the path") && text.contains("as external") {
    let name = package.split('/').take(if package.starts_with('@') { 2 } else { 1 }).collect::<Vec<_>>().join("/");
    if package.starts_with('.') || package.starts_with('/') {
      Some("Imports resolve from node_modules next to data/; a relative path is relative to the generated file.".to_string())
    } else {
      Some(format!("Install it next to data/: npm install {}", name))
    }
  } else if text.contains("--platform") || text.contains("--packages") {
    None
  } else {
    Some(text.to_string())
  }
}

#[derive(Debug, Clone)]
pub enum BundleError {
  /// No esbuild: `$FLOWLANG_ESBUILD`, `node_modules/.bin/esbuild` next to
  /// `data/`, or `esbuild` on the PATH.
  NoEsbuild,
  /// esbuild ran and reported errors.
  Failed(Vec<Diagnostic>),
  /// Anything else: a file that could not be written or read.
  Other(String),
}

impl BundleError {
  /// The error as one message, with locations in the author's terms.
  pub fn text(&self, layout: &Layout) -> String {
    match self {
      BundleError::NoEsbuild => "the command imports modules or is TypeScript, so it runs from an esbuild bundle, and there is no bundle for its current code here and no esbuild to build one: run `npm install esbuild` next to data/ (or set FLOWLANG_ESBUILD), or deploy with the bundles built where esbuild is (`newbound jsbundle`, the js package variant)".to_string(),
      BundleError::Other(e) => e.clone(),
      BundleError::Failed(ds) => ds.iter().map(|d| d.full(d.line.map(|l| layout.describe(l)).as_deref()))
        .collect::<Vec<_>>().join("\n"),
    }
  }
}

/// The bundle for a command, `<cmd>.bundle.js` next to its generated file
/// in the library crate's `src/` (`src_root`).
///
/// A bundle records what it was built from in its first line:
/// `// flowlang-bundle <source key> <packages key>` - the command's code,
/// and the content of the npm lockfiles next to data/ (`-` when there are
/// none). The bundle on disk is used when its source key matches the
/// command and either its packages key matches or there is no lockfile here
/// - a deployment that ships pre-built bundles needs neither esbuild nor
/// node_modules. Otherwise esbuild rebuilds it (writing the generated file
/// first). Without esbuild, a bundle whose source key matches is still used
/// (with a warning, once, when the lockfile here differs); one built from
/// other code is an error.
pub fn bundle(store: &DataStore, src_root: &Path, lib: &str, ctl: &str, cmd: &str, src: &JsSource) -> Result<std::sync::Arc<Bundle>, BundleError> {
  use std::collections::HashMap;
  use std::sync::{Arc, Mutex};
  static CACHE: Mutex<Option<HashMap<std::path::PathBuf, Arc<Bundle>>>> = Mutex::new(None);
  static BUILDING: Mutex<()> = Mutex::new(());
  static WARNED: Mutex<Option<std::collections::HashSet<std::path::PathBuf>>> = Mutex::new(None);

  let dir = src_root.join(lib).join(ctl);
  let entry = dir.join(format!("{}.{}", cmd, src.extension()));
  let out = dir.join(format!("{}.bundle.js", cmd));
  let source = source_key(src);
  let packages = packages_key(store);
  let source_name = format!("flowlang:{}/{}/{}.{}", lib, ctl, cmd, src.extension());
  let file_name = format!("{}.{}", cmd, src.extension());
  // Usable: built from this code, and from these packages unless there are
  // no lockfiles here to say otherwise.
  let usable = |b: &Bundle| -> bool {
    let (s, p) = b.key.split_once('-').unwrap_or((b.key.as_str(), ""));
    s == source && (packages.is_none() || packages.as_deref() == Some(p))
  };
  let cached = || -> Option<Arc<Bundle>> {
    let guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    guard.as_ref().and_then(|c| c.get(&out)).filter(|b| usable(b)).cloned()
  };
  if let Some(b) = cached() { return Ok(b); }

  let _building = BUILDING.lock().unwrap_or_else(|e| e.into_inner());
  if let Some(b) = cached() { return Ok(b); }
  let remember = |b: Bundle| -> Arc<Bundle> {
    let b = Arc::new(b);
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    guard.get_or_insert_with(HashMap::new).insert(out.clone(), b.clone());
    b
  };
  // The bundle on disk, with the key its banner records.
  let on_disk = read_to_string(&out).ok().and_then(|text| {
    let mut words = text.lines().next()?.strip_prefix("// flowlang-bundle ")?.split(' ');
    let key = format!("{}-{}", words.next()?, words.next()?);
    Some((key, text))
  });
  // A bundle that does not split (truncated, hand-edited) counts as absent.
  let existing = on_disk.and_then(|(key, text)| split_bundle(&text, &file_name, &source_name, key).ok());
  if let Some(b) = existing.as_ref().filter(|b| usable(b)) {
    return Ok(remember(b.clone()));
  }

  let packages_word = packages.clone().unwrap_or_else(|| "-".to_string());
  let banner = format!("// flowlang-bundle {} {}", source, packages_word);
  write_if_changed(&entry, &command_file(src, "../../flowlang.d.ts"));
  write_if_changed(&src_root.join("flowlang.d.ts"), &flowlang_dts(store));
  match run_esbuild(store, &entry, &out, &banner) {
    Ok(()) => {
      let text = read_to_string(&out).map_err(|e| BundleError::Other(format!("could not read the bundle {}: {}", out.display(), e)))?;
      Ok(remember(split_bundle(&text, &file_name, &source_name, format!("{}-{}", source, packages_word))?))
    }
    Err(BundleError::NoEsbuild) => {
      // Built from this code against other packages: still the best there is.
      match existing.filter(|b| b.key.split_once('-').map(|(s, _)| s == source).unwrap_or(false)) {
        Some(b) => {
          let mut warned = WARNED.lock().unwrap_or_else(|e| e.into_inner());
          if warned.get_or_insert_with(Default::default).insert(out.clone()) {
            eprintln!("[js] {}/{}/{}: its bundle was built against other npm packages than the lockfile here describes, and there is no esbuild to rebuild it - using it as built", lib, ctl, cmd);
          }
          Ok(remember(b))
        }
        None => Err(BundleError::NoEsbuild),
      }
    }
    Err(e) => Err(e),
  }
}

/// Identifies a command's code: everything in the store that its bundle is
/// built from.
fn source_key(src: &JsSource) -> String {
  use std::collections::hash_map::DefaultHasher;
  use std::hash::{Hash, Hasher};
  let mut h = DefaultHasher::new();
  (&src.lang, &src.imports, &src.params, &src.returntype, &src.body, &src.name).hash(&mut h);
  format!("{:016x}", h.finish())
}

/// Identifies the installed npm packages by the content of the lockfiles
/// next to data/; `None` when there are none. A lockfile's hash is
/// recomputed only when its size or modification time changes.
fn packages_key(store: &DataStore) -> Option<String> {
  use std::collections::hash_map::DefaultHasher;
  use std::collections::HashMap;
  use std::hash::{Hash, Hasher};
  use std::sync::Mutex;
  static MEMO: Mutex<Option<HashMap<std::path::PathBuf, (u64, Option<std::time::SystemTime>, u64)>>> = Mutex::new(None);
  let parent = store.root.canonicalize().ok().and_then(|r| r.parent().map(|p| p.to_path_buf()))?;
  let mut h = DefaultHasher::new();
  let mut any = false;
  for f in ["package-lock.json", "node_modules/.package-lock.json", "yarn.lock", "pnpm-lock.yaml"] {
    let path = parent.join(f);
    let Ok(meta) = std::fs::metadata(&path) else { continue };
    let stamp = (meta.len(), meta.modified().ok());
    let mut memo = MEMO.lock().unwrap_or_else(|e| e.into_inner());
    let memo = memo.get_or_insert_with(HashMap::new);
    let content = match memo.get(&path) {
      Some((len, modified, hash)) if (*len, *modified) == stamp => *hash,
      _ => {
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let mut fh = DefaultHasher::new();
        bytes.hash(&mut fh);
        let hash = fh.finish();
        memo.insert(path.clone(), (stamp.0, stamp.1, hash));
        hash
      }
    };
    (f, content).hash(&mut h);
    any = true;
  }
  if any { Some(format!("{:016x}", h.finish())) } else { None }
}

/// Where esbuild is looked for: `$FLOWLANG_ESBUILD` alone when it is set
/// (an explicit choice; a path that does not exist means none), else
/// `node_modules/.bin/esbuild` next to data/, then `esbuild` on the PATH.
fn esbuild_candidates(store: &DataStore) -> Vec<std::path::PathBuf> {
  if let Some(t) = std::env::var("FLOWLANG_ESBUILD").ok().filter(|t| !t.trim().is_empty()) {
    return vec![t.into()];
  }
  let mut v: Vec<std::path::PathBuf> = Vec::new();
  if let Some(parent) = store.root.canonicalize().ok().and_then(|r| r.parent().map(|p| p.to_path_buf())) {
    v.push(parent.join("node_modules").join(".bin").join("esbuild"));
  }
  v.push("esbuild".into());
  v
}

/// Every JavaScript/TypeScript command in the store that runs from a bundle,
/// as (lib, ctl, cmd, id).
pub fn bundled_commands(store: &DataStore) -> Vec<(String, String, String, String)> {
  let mut out = Vec::new();
  let mut libs: Vec<String> = std::fs::read_dir(&store.root)
    .map(|it| it.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect())
    .unwrap_or_default();
  libs.retain(|l| store.exists(l, "controls"));
  libs.sort();
  for lib in libs {
    let found = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      let mut v = Vec::new();
      for c in store.get_data(&lib, "controls").get_object("data").get_array("list").objects() {
        let c = c.object();
        let (ctl, ctl_id) = (c.get_string("name"), c.get_string("id"));
        if !store.exists(&lib, &ctl_id) { continue; }
        let data = store.get_data(&lib, &ctl_id).get_object("data");
        if !data.has("cmd") { continue; }
        for e in data.get_array("cmd").objects() {
          let e = e.object();
          let (cmd, id) = (e.get_string("name"), e.get_string("id"));
          if JsSource::read(store, &lib, &id).map_or(false, |s| s.needs_bundle()) {
            v.push((lib.clone(), ctl.clone(), cmd, id));
          }
        }
      }
      v
    })).unwrap_or_default();
    out.extend(found);
  }
  out
}

/// Builds (or confirms current) the bundle of every command that runs from
/// one - what a package ships so that its deployments need no esbuild.
/// Returns the commands done, as `lib:ctl:cmd`, and the failures with their
/// messages.
pub fn build_bundles(store: &DataStore) -> (Vec<String>, Vec<(String, String)>) {
  let mut done = Vec::new();
  let mut failed = Vec::new();
  for (lib, ctl, cmd, id) in bundled_commands(store) {
    let label = format!("{}:{}:{}", lib, ctl, cmd);
    let Some(src) = JsSource::read(store, &lib, &id) else { continue };
    let src_root = store.get_lib_root(&lib).join("src");
    match bundle(store, &src_root, &lib, &ctl, &cmd, &src) {
      Ok(_) => done.push(label),
      Err(e) => failed.push((label, e.text(&layout(&src)))),
    }
  }
  (done, failed)
}

/// The node_modules next to data/, for esbuild's NODE_PATH.
fn node_modules(store: &DataStore) -> Option<std::path::PathBuf> {
  store.root.canonicalize().ok().and_then(|r| r.parent().map(|p| p.join("node_modules")))
}

fn run_esbuild(store: &DataStore, entry: &Path, out: &Path, banner: &str) -> Result<(), BundleError> {
  for exe in esbuild_candidates(store) {
    if exe.components().count() > 1 && !exe.exists() { continue; }
    let mut command = std::process::Command::new(&exe);
    command.arg(entry)
      .args(["--bundle", "--format=iife", "--global-name=__flowlang_module", "--platform=neutral",
             "--main-fields=module,main", "--target=es2022", "--sourcemap=inline", "--sources-content=false",
             "--log-level=error", "--color=false", "--log-limit=0"])
      .arg(format!("--banner:js={}", banner))
      .arg(format!("--outfile={}", out.display()));
    if let Some(nm) = node_modules(store) { command.env("NODE_PATH", nm); }
    let output = match command.output() {
      Ok(o) => o,
      Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
      Err(e) => return Err(BundleError::Other(format!("could not start {}: {}", exe.display(), e))),
    };
    if output.status.success() { return Ok(()); }
    let text = String::from_utf8_lossy(&output.stderr).to_string();
    let file = entry.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
    let ds = parse_esbuild_errors(&text, &file);
    return Err(if ds.is_empty() { BundleError::Other(format!("esbuild failed: {}", text.trim())) } else { BundleError::Failed(ds) });
  }
  Err(BundleError::NoEsbuild)
}

/// esbuild's error report, as diagnostics:
///
/// ```text
/// ✘ [ERROR] Could not resolve "fs"
///
///     src/lib/ctl/cmd.js:3:15:
///       3 │ import fs from "fs";
///         ╵                ~~~~
///
///   The package "fs" wasn't found on the file system but is built into node. ...
/// ```
///
/// A location in the generated file (`file`) keeps its line; a note is
/// appended to its error's message.
pub(crate) fn parse_esbuild_errors(text: &str, file: &str) -> Vec<Diagnostic> {
  let mut out: Vec<Diagnostic> = Vec::new();
  for line in text.lines() {
    let t = line.trim();
    if let Some(msg) = t.strip_prefix("✘ [ERROR] ").or_else(|| t.strip_prefix("X [ERROR] ")) {
      out.push(Diagnostic { line: None, column: None, message: msg.to_string(), notes: Vec::new() });
      continue;
    }
    let Some(d) = out.last_mut() else { continue };
    if t.is_empty() || t.contains('│') || t.contains('╵') || t.contains('╷') { continue; }
    // A location: "path:line:col:".
    if let Some(loc) = t.strip_suffix(':') {
      let mut it = loc.rsplitn(3, ':');
      if let (Some(col), Some(ln), Some(path)) = (it.next(), it.next(), it.next()) {
        if let (Ok(ln), Ok(col)) = (ln.parse::<usize>(), col.parse::<usize>()) {
          if d.line.is_none() && (path.ends_with(file) || path.replace('\\', "/").ends_with(file)) {
            d.line = Some(ln);
            d.column = Some(col);
          } else if d.line.is_none() {
            d.message.push_str(&format!(" (in {}:{}:{})", path.rsplit_once("node_modules/").map(|x| x.1).unwrap_or(path), ln, col));
          }
          continue;
        }
      }
    }
    // A note: a paragraph may wrap over several lines.
    let fresh = line.starts_with("  ") && !line.starts_with("    ") || d.notes.is_empty();
    if fresh {
      d.notes.push(t.to_string());
    } else if let Some(last) = d.notes.last_mut() {
      last.push(' ');
      last.push_str(t);
    }
  }
  for d in out.iter_mut() {
    let message = d.message.clone();
    d.notes = d.notes.iter().filter_map(|n| note(n, &message)).collect();
  }
  out
}

/// Splits esbuild's output into the code and its inline source map, renaming
/// the map's sources: the generated file as `source_name`, packages as
/// `npm:<path inside node_modules>`.
fn split_bundle(text: &str, file_name: &str, source_name: &str, key: String) -> Result<Bundle, BundleError> {
  const MARK: &str = "//# sourceMappingURL=data:application/json;base64,";
  let (code, encoded) = text.rsplit_once(MARK)
    .ok_or_else(|| BundleError::Other("the bundle has no inline source map".to_string()))?;
  let raw = std::panic::catch_unwind(|| crate::base64::Base64::decode(encoded.trim().chars().collect()))
    .map_err(|_| BundleError::Other("the bundle's source map is not valid base64".to_string()))?;
  let json = String::from_utf8(raw).map_err(|_| BundleError::Other("the bundle's source map is not UTF-8".to_string()))?;
  let mut map = ndata::dataobject::DataObject::try_from_string(&json)
    .map_err(|_| BundleError::Other("the bundle's source map is not JSON".to_string()))?;
  let mut renamed = ndata::dataarray::DataArray::new();
  if map.has("sources") {
    for s in map.get_array("sources").objects() {
      let s = s.string();
      let name = if s == file_name || s.ends_with(&format!("/{}", file_name)) {
        source_name.to_string()
      } else if let Some((_, inside)) = s.rsplit_once("node_modules/") {
        format!("npm:{}", inside)
      } else {
        format!("{}/{}", source_name.rsplit_once('/').map(|x| x.0).unwrap_or(source_name), s.trim_start_matches("./"))
      };
      renamed.push_string(&name);
    }
  }
  map.put_array("sources", renamed);
  Ok(Bundle { key, code: code.to_string(), map: map.to_string().into_bytes(), source_name: source_name.to_string() })
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
    let impl_key = if cmd.has(&typ) { typ.clone() } else if let Some(k) = record_key(&cmd, &typ) { k.to_string() } else { continue };
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
    if is_lang(&typ) && JS_LANGS.iter().chain(TS_LANGS.iter()).any(|k| impl_data.has(k) && uses_await(&impl_data.get_string(k))) {
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
      lang: "javascript".into(),
      params: params.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
      returntype: returntype.into(),
      imports: String::new(),
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
    assert!(f.contains("\nexport default function cmd(who, n) {\nreturn {who, n};\n}\n"), "{}", f);
    let a = command_file(&src(&[], "String", "return await f()"), "x.d.ts");
    assert!(a.contains(" * @returns {Promise<string>}\n") && a.contains("\nexport default async function cmd() {\n"), "{}", a);
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
  fn imports_come_first_and_lines_are_described_in_the_authors_terms() {
    let mut s = src(&[("n", "Integer")], "Any", "const x = _.range(n);\nreturn x;");
    s.imports = "import _ from \"lodash\";\nimport { a } from \"./a.js\";".into();
    let f = command_file(&s, "flowlang.d.ts");
    let lines: Vec<&str> = f.lines().collect();
    let l = layout(&s);
    assert_eq!(lines[l.imports_first - 1], "import _ from \"lodash\";", "{}", f);
    assert_eq!(lines[l.imports_first], "import { a } from \"./a.js\";");
    assert_eq!(lines[l.body_first - 1], "const x = _.range(n);");
    assert_eq!(l.describe(l.imports_first + 1), "imports line 2");
    assert_eq!(l.describe(l.body_first + 1), "body line 2");
    assert_eq!(l.describe(l.body_first + 2), "the end of the body");
    assert!(s.needs_bundle() && !src(&[], "Any", "return 1").needs_bundle());
  }

  #[test]
  fn typescript_writes_its_types_out() {
    let mut s = src(&[("who", "String"), ("n", "Integer")], "String", "return await f(who, n)");
    s.lang = "typescript".into();
    let f = command_file(&s, "flowlang.d.ts");
    assert!(!f.contains("@ts-check") && !f.contains("@param"), "{}", f);
    assert!(f.contains("\nexport default async function cmd(who: string, n: number): Promise<string> {\nreturn await f(who, n)\n}\n"), "{}", f);
    assert_eq!(s.extension(), "ts");
    assert!(s.needs_bundle());
    assert_eq!(layout(&s).describe(layout(&s).body_first), "body line 1");
  }

  #[test]
  fn esbuild_errors_become_diagnostics() {
    let text = "✘ [ERROR] Could not resolve \"fs\"\n\n    src/lib/ctl/cmd.js:4:15:\n      4 │ import fs from \"fs\";\n        ╵                ~~~~\n\n  The package \"fs\" wasn't found on the file system but is built into node.\n\n✘ [ERROR] Expected \";\" but found \")\"\n\n    node_modules/x/index.js:1:2:\n      1 │ a)\n\n";
    let d = parse_esbuild_errors(text, "cmd.js");
    assert_eq!(d.len(), 2, "{:?}", d);
    assert_eq!((d[0].line, d[0].column), (Some(4), Some(15)));
    assert_eq!(d[0].message, "Could not resolve \"fs\"");
    assert_eq!(d[0].notes.len(), 1, "{:?}", d[0]);
    assert!(d[0].notes[0].contains("Node's built-in modules") && !d[0].notes[0].contains("--platform"), "{:?}", d[0]);
    assert_eq!(d[0].full(Some("imports line 1")).lines().next(), Some("Could not resolve \"fs\" (imports line 1)"));
    assert_eq!(d[1].line, None);
    assert!(d[1].message.contains("(in x/index.js:1:2)"), "{:?}", d[1]);
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
