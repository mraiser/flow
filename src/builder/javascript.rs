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
  let modules = shared_modules(store, src, (lib, ctl, cmd)).map_err(|(spec, why)| {
    let lay = layout(src);
    let line = src.imports.lines().position(|l| l.contains(spec.as_str())).map(|i| lay.imports_first + i);
    BundleError::Failed(vec![Diagnostic { line, column: None, message: format!("cannot import \"{}\": {}", spec, why), notes: Vec::new() }])
  })?;
  let source = bundle_key(src, &modules);
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
  write_modules(store, &modules);
  match run_esbuild(store, &entry, &out, &banner, &modules) {
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

/// Another JavaScript or TypeScript command imported as a module:
/// `import other, { helper } from "flowlang:<lib>/<ctl>/<cmd>"`. Its default
/// export is the command's function; whatever its imports section exports
/// comes with it. The bundler and tsc find it at `path`, its generated file.
pub struct SharedModule {
  /// `flowlang:<lib>/<ctl>/<cmd>`
  pub specifier: String,
  pub src: JsSource,
  pub path: std::path::PathBuf,
}

/// The prefix of an import that names a command.
pub const MODULE_PREFIX: &str = "flowlang:";

/// The `flowlang:` specifiers in an imports section, in order, once each.
pub fn command_imports(imports: &str) -> Vec<String> {
  let mut found: Vec<String> = Vec::new();
  for q in ['"', '\'', '`'] {
    let opening = format!("{}{}", q, MODULE_PREFIX);
    let mut rest = imports;
    while let Some(i) = rest.find(&opening) {
      let after = &rest[i + 1..];
      let Some(end) = after.find(q) else { break };
      let spec = after[..end].to_string();
      if !found.contains(&spec) { found.push(spec); }
      rest = &after[end..];
    }
  }
  // In the order they appear.
  found.sort_by_key(|s| imports.find(s.as_str()).unwrap_or(usize::MAX));
  found
}

/// The commands `src` (the command `lib:ctl:cmd`) imports, directly or
/// through each other, each once. An import that names no JavaScript or
/// TypeScript command is an error: (the specifier, why).
pub fn shared_modules(store: &DataStore, src: &JsSource, own: (&str, &str, &str)) -> Result<Vec<SharedModule>, (String, String)> {
  let own_spec = format!("{}{}/{}/{}", MODULE_PREFIX, own.0, own.1, own.2);
  let mut out: Vec<SharedModule> = Vec::new();
  let mut queue: Vec<String> = command_imports(&src.imports);
  let mut i = 0;
  while i < queue.len() {
    let spec = queue[i].clone();
    i += 1;
    if spec == own_spec || out.iter().any(|m| m.specifier == spec) { continue; }
    let m = resolve_module(store, &spec).map_err(|why| (spec.clone(), why))?;
    for s in command_imports(&m.src.imports) {
      if !queue.contains(&s) { queue.push(s); }
    }
    out.push(m);
  }
  Ok(out)
}

/// `resolve_module`, remembered: an import is resolved again only when one
/// of the records it was read from (the library's meta and controls, the
/// control, the command and its implementation) changes size or time.
fn resolve_module(store: &DataStore, spec: &str) -> Result<SharedModule, String> {
  use std::collections::HashMap;
  use std::sync::Mutex;
  type Stamp = Vec<(std::path::PathBuf, Option<(u64, Option<std::time::SystemTime>)>)>;
  static MEMO: Mutex<Option<HashMap<(std::path::PathBuf, String), (Stamp, JsSource, std::path::PathBuf)>>> = Mutex::new(None);
  let stamp_of = |files: &[std::path::PathBuf]| -> Stamp {
    files.iter().map(|f| (f.clone(), std::fs::metadata(f).ok().map(|m| (m.len(), m.modified().ok())))).collect()
  };
  let key = (store.root.clone(), spec.to_string());
  if let Some((stamp, src, path)) = MEMO.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(&key)) {
    let files: Vec<std::path::PathBuf> = stamp.iter().map(|(f, _)| f.clone()).collect();
    if stamp_of(&files) == *stamp {
      return Ok(SharedModule { specifier: spec.to_string(), src: src.clone(), path: path.clone() });
    }
  }
  let (m, files) = resolve_module_from_store(store, spec)?;
  let stamp = stamp_of(&files);
  MEMO.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new)
    .insert(key, (stamp, m.src.clone(), m.path.clone()));
  Ok(m)
}

/// Resolves an import from the store; also returns the record files it read.
fn resolve_module_from_store(store: &DataStore, spec: &str) -> Result<(SharedModule, Vec<std::path::PathBuf>), String> {
  let name = &spec[MODULE_PREFIX.len()..];
  let parts: Vec<&str> = name.split('/').collect();
  if parts.len() != 3 || parts.iter().any(|p| p.is_empty()) {
    return Err(format!("a command is imported as \"{}<lib>/<ctl>/<cmd>\"", MODULE_PREFIX));
  }
  let (lib, ctl, cmd) = (parts[0], parts[1], parts[2]);
  let no_such = || format!("there is no command {}:{}:{}", lib, ctl, cmd);
  // The ids on the way to the command, for the memo's stamp.
  let (ctl_id, id) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    if !store.exists(lib, "controls") { return None; }
    let list = store.get_data(lib, "controls").get_object("data").get_array("list");
    let ctl_id = list.objects().into_iter().map(|c| c.object()).find(|c| c.get_string("name") == ctl)?.get_string("id");
    let cmds = store.get_data(lib, &ctl_id).get_object("data").get_array("cmd");
    let id = cmds.objects().into_iter().map(|c| c.object()).find(|c| c.get_string("name") == cmd)?.get_string("id");
    if store.exists(lib, &id) { Some((ctl_id, id)) } else { None }
  })).ok().flatten().ok_or_else(no_such)?;
  let record = store.get_data(lib, &id).get_object("data");
  let typ = record.get_string("type");
  if !is_lang(&typ) {
    return Err(format!("{}:{}:{} is a {} command; only JavaScript and TypeScript commands can be imported (call it with flowlang.call)", lib, ctl, cmd, if typ.is_empty() { "non-JavaScript" } else { typ.as_str() }));
  }
  let src = JsSource::read(store, lib, &id).ok_or_else(|| format!("{}:{}:{} has no JavaScript body", lib, ctl, cmd))?;
  let path = store.get_lib_root(lib).join("src").join(lib).join(ctl).join(format!("{}.{}", cmd, src.extension()));
  let impl_id = if record.has(&typ) { record.get_string(&typ) } else { String::new() };
  let files = vec![
    store.root.join(lib).join("meta.json"),
    store.get_data_file(lib, "controls"),
    store.get_data_file(lib, &ctl_id),
    store.get_data_file(lib, &id),
    store.get_data_file(lib, &impl_id),
  ];
  Ok((SharedModule { specifier: spec.to_string(), src, path }, files))
}

/// Writes the imported commands' generated files (and the flowlang.d.ts
/// beside them), so the bundler and tsc can read them.
pub fn write_modules(store: &DataStore, modules: &[SharedModule]) {
  for m in modules {
    write_if_changed(&m.path, &command_file(&m.src, "../../flowlang.d.ts"));
    if let Some(root) = m.path.parent().and_then(|p| p.parent()).and_then(|p| p.parent()) {
      write_if_changed(&root.join("flowlang.d.ts"), &flowlang_dts(store));
    }
  }
}

/// `compilerOptions.paths` for the imported commands: each specifier to its
/// generated file, by absolute path.
pub fn module_paths(modules: &[SharedModule]) -> ndata::dataobject::DataObject {
  let mut paths = ndata::dataobject::DataObject::new();
  for m in modules {
    let file = m.path.canonicalize().unwrap_or_else(|_| m.path.clone());
    let mut a = ndata::dataarray::DataArray::new();
    a.push_string(&file.to_string_lossy());
    paths.put_array(&m.specifier, a);
  }
  paths
}

/// A bundle's key: its own code and that of every command it imports.
fn bundle_key(src: &JsSource, modules: &[SharedModule]) -> String {
  if modules.is_empty() { return source_key(src); }
  use std::collections::hash_map::DefaultHasher;
  use std::hash::{Hash, Hasher};
  let mut h = DefaultHasher::new();
  source_key(src).hash(&mut h);
  for m in modules { (&m.specifier, source_key(&m.src)).hash(&mut h); }
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

fn run_esbuild(store: &DataStore, entry: &Path, out: &Path, banner: &str, modules: &[SharedModule]) -> Result<(), BundleError> {
  let tsconfig = module_tsconfig(modules, &out.with_extension("tsconfig.json"))?;
  let result = run_esbuild_with(store, Input::File(entry), out, banner, tsconfig.as_deref());
  if let Some(p) = tsconfig { let _ = std::fs::remove_file(p); }
  result
}

/// Imported commands resolve through a tsconfig naming each one's file:
/// written at `path` when there are any.
fn module_tsconfig(modules: &[SharedModule], path: &Path) -> Result<Option<std::path::PathBuf>, BundleError> {
  if modules.is_empty() { return Ok(None); }
  let mut options = ndata::dataobject::DataObject::new();
  options.put_object("paths", module_paths(modules));
  let mut config = ndata::dataobject::DataObject::new();
  config.put_object("compilerOptions", options);
  std::fs::write(path, config.to_string())
    .map_err(|e| BundleError::Other(format!("could not write {}: {}", path.display(), e)))?;
  Ok(Some(path.to_path_buf()))
}

/// What esbuild bundles: a file, or source given on stdin under a file name,
/// whose imports resolve from a directory.
enum Input<'a> {
  File(&'a Path),
  Stdin { text: &'a str, dir: &'a Path, name: &'a str },
}

fn run_esbuild_with(store: &DataStore, input: Input, out: &Path, banner: &str, tsconfig: Option<&Path>) -> Result<(), BundleError> {
  for exe in esbuild_candidates(store) {
    if exe.components().count() > 1 && !exe.exists() { continue; }
    let mut command = std::process::Command::new(&exe);
    let file = match &input {
      Input::File(entry) => {
        command.arg(entry).stdin(std::process::Stdio::null());
        entry.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()
      }
      Input::Stdin { dir, name, .. } => {
        command.arg(format!("--sourcefile={}", name)).current_dir(dir).stdin(std::process::Stdio::piped());
        name.to_string()
      }
    };
    command
      .args(["--bundle", "--format=iife", "--global-name=__flowlang_module", "--platform=neutral",
             "--main-fields=module,main", "--target=es2022", "--sourcemap=inline", "--sources-content=false",
             "--log-level=error", "--color=false", "--log-limit=0"])
      .arg(format!("--banner:js={}", banner))
      .arg(format!("--outfile={}", out.display()));
    if let Some(t) = tsconfig { command.arg(format!("--tsconfig={}", t.display())); }
    if let Some(nm) = node_modules(store) { command.env("NODE_PATH", nm); }
    command.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut child = match command.spawn() {
      Ok(c) => c,
      Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
      Err(e) => return Err(BundleError::Other(format!("could not start {}: {}", exe.display(), e))),
    };
    if let Input::Stdin { text, .. } = &input {
      use std::io::Write;
      // Dropped at the end of the statement: esbuild sees the end of input.
      let _ = child.stdin.take().map(|mut i| i.write_all(text.as_bytes()));
    }
    let output = child.wait_with_output()
      .map_err(|e| BundleError::Other(format!("{} did not finish: {}", exe.display(), e)))?;
    if output.status.success() { return Ok(()); }
    let text = String::from_utf8_lossy(&output.stderr).to_string();
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
  split_bundle_naming(text, source_name, key, &|s: &str| command_source_name(s, file_name, source_name))
}

/// A command bundle's name for a source in its map: the command itself,
/// `npm:<path>` for a package, `flowlang:<lib>/<ctl>/<cmd>.<ext>` for an
/// imported command.
fn command_source_name(s: &str, file_name: &str, source_name: &str) -> String {
  if s == file_name || s.ends_with(&format!("/{}", file_name)) {
    source_name.to_string()
  } else if let Some((_, inside)) = s.rsplit_once("node_modules/") {
    format!("npm:{}", inside)
  } else {
    // Anything else is a generated command file, <lib>/<ctl>/<cmd>.<ext>
    // under its library's root: an imported command. The map's paths are
    // relative to the bundle's directory, <lib>/<ctl>: resolve them from
    // there.
    let dir = source_name.strip_prefix(MODULE_PREFIX).unwrap_or(source_name).rsplit_once('/').map(|x| x.0).unwrap_or("");
    let mut parts: Vec<&str> = dir.split('/').filter(|p| !p.is_empty()).collect();
    for p in s.split('/') {
      match p {
        "" | "." => {}
        ".." => { parts.pop(); }
        p => parts.push(p),
      }
    }
    format!("{}{}", MODULE_PREFIX, parts[parts.len().saturating_sub(3)..].join("/"))
  }
}

fn split_bundle_naming(text: &str, source_name: &str, key: String, name: &dyn Fn(&str) -> String) -> Result<Bundle, BundleError> {
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
      let name = name(&s);
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

// ---- flow jstest files that import -----------------------------------------------------------

/// A test file that imports, as the module esbuild bundles. Its import
/// statements move to line 1 (their own lines left blank), followed there by
/// the opening of the async function the rest of the file is the body of -
/// so every other line keeps its number, `test`/`assert` stay locals, and
/// top-level `await` still works.
pub struct TestEntry {
  pub text: String,
  /// The import statements, joined.
  pub imports: String,
  /// Each import: its first line in the file, and the columns it occupies
  /// on line 1 of `text`.
  pub spans: Vec<(usize, usize, usize)>,
}

fn starts_import(t: &str) -> bool {
  t.strip_prefix("import").and_then(|r| r.chars().next()).map_or(false, |c| matches!(c, ' ' | '\t' | '{' | '*' | '"' | '\''))
}

/// True once an import statement's text names its module.
fn import_complete(stmt: &str) -> bool {
  let quoted = |r: &str| {
    let r = r.trim_start();
    r.chars().next().filter(|q| *q == '"' || *q == '\'').map_or(false, |q| r[1..].contains(q))
  };
  let rest = stmt.trim_start()["import".len()..].trim_start();
  if quoted(rest) { return true; }
  let mut from = rest;
  while let Some(i) = from.find("from") {
    if quoted(&from[i + 4..]) { return true; }
    from = &from[i + 4..];
  }
  false
}

/// The entry for a test file, or None when it has no import statements.
/// An error is (file line, message).
pub fn test_entry(source: &str) -> Result<Option<TestEntry>, (usize, String)> {
  let mut lines: Vec<String> = source.split('\n').map(str::to_string).collect();
  let mut statements: Vec<(usize, String)> = Vec::new();
  let mut i = 0;
  let mut in_comment = false;
  // The header: imports, blank lines and comments.
  while i < lines.len() {
    let t = lines[i].trim().to_string();
    if in_comment { if t.contains("*/") { in_comment = false; } i += 1; continue; }
    if t.is_empty() || t.starts_with("//") { i += 1; continue; }
    if t.starts_with("/*") { in_comment = !t.contains("*/"); i += 1; continue; }
    if !starts_import(&t) { break; }
    let first = i;
    let mut stmt = t;
    while !import_complete(&stmt) {
      i += 1;
      if i >= lines.len() { return Err((first + 1, "this import statement does not end".to_string())); }
      stmt.push(' ');
      stmt.push_str(lines[i].trim());
    }
    for l in &mut lines[first..=i] { l.clear(); }
    if !stmt.trim_end().ends_with(';') { stmt.push(';'); }
    statements.push((first + 1, stmt));
    i += 1;
  }
  for (n, l) in lines.iter().enumerate().skip(i) {
    if starts_import(l) {
      return Err((n + 1, "imports must come first in a test file, before its tests and other statements".to_string()));
    }
  }
  if statements.is_empty() { return Ok(None); }
  let mut first_line = String::new();
  let mut spans = Vec::new();
  for (line, stmt) in &statements {
    let start = first_line.len();
    first_line.push_str(stmt);
    spans.push((*line, start, first_line.len()));
    first_line.push(' ');
  }
  let imports = first_line.trim_end().to_string();
  first_line.push_str("export default async function () { const { test, assert } = __flowlang.testing.begin(); ");
  first_line.push_str(&lines[0]);
  let mut text = first_line;
  for l in &lines[1..] { text.push('\n'); text.push_str(l); }
  text.push_str("\n;return await __flowlang.testing.finish(); }\n");
  Ok(Some(TestEntry { text, imports, spans }))
}

/// Bundles a test file that imports - npm packages from node_modules next to
/// data/ (or above the file), commands as `flowlang:<lib>/<ctl>/<cmd>`, and
/// files relative to it. None when it does not import. `source_name` is
/// the file's name in stack traces. Errors are ready to print, at the
/// file's own lines.
pub fn bundle_test(store: &DataStore, file: &Path, source: &str, source_name: &str) -> Result<Option<Bundle>, String> {
  let entry = match test_entry(source) {
    Ok(Some(e)) => e,
    Ok(None) => return Ok(None),
    Err((line, msg)) => return Err(format!("{} (line {})", msg, line)),
  };
  let line_of = |needle: &str| entry.spans.iter()
    .find(|(_, a, b)| entry.imports.get(*a..*b).map_or(false, |stmt| stmt.contains(needle))).map(|(l, _, _)| *l);
  let probe = JsSource {
    name: "test".to_string(), lang: "javascript".to_string(), params: Vec::new(),
    returntype: "Any".to_string(), imports: entry.imports.clone(), body: String::new(),
  };
  let modules = shared_modules(store, &probe, ("", "", "")).map_err(|(spec, why)| {
    let at = line_of(&spec).map(|l| format!(" (line {})", l)).unwrap_or_default();
    format!("cannot import \"{}\": {}{}", spec, why, at)
  })?;
  write_modules(store, &modules);

  let dir = file.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
  let dir = dir.canonicalize().map_err(|e| format!("{}: {}", dir.display(), e))?;
  let name = file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
  let key = {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    (&entry.text, std::process::id(), source_name).hash(&mut h);
    format!("{:016x}", h.finish())
  };
  let out = std::env::temp_dir().join(format!("flowlang_jstest_{}.js", key));
  let failed = |e: BundleError| -> String {
    match e {
      BundleError::NoEsbuild => "this test file imports, so it runs from an esbuild bundle, and there is no esbuild: run `npm install esbuild` next to data/ (or set FLOWLANG_ESBUILD)".to_string(),
      BundleError::Other(m) => m,
      BundleError::Failed(ds) => ds.iter().map(|d| {
        // Line 1 holds the imports: back to the line each came from.
        let line = match (d.line, d.column) {
          (Some(1), Some(c)) => entry.spans.iter().find(|(_, a, b)| c >= *a && c < *b).map(|(l, _, _)| *l).or(Some(1)),
          (l, _) => l,
        };
        d.full(line.map(|l| format!("line {}", l)).as_deref())
      }).collect::<Vec<_>>().join("\n"),
    }
  };
  let tsconfig = module_tsconfig(&modules, &out.with_extension("tsconfig.json")).map_err(failed)?;
  let built = run_esbuild_with(store, Input::Stdin { text: &entry.text, dir: &dir, name: &name }, &out, "// flowlang-jstest", tsconfig.as_deref());
  if let Some(t) = tsconfig { let _ = std::fs::remove_file(t); }
  built.map_err(failed)?;
  let text = read_to_string(&out).map_err(|e| format!("could not read the bundle {}: {}", out.display(), e));
  let _ = std::fs::remove_file(&out);
  let text = text?;

  // The map's paths are relative to the bundle's directory.
  let out_dir = out.parent().map(Path::to_path_buf).unwrap_or_default();
  let shown_dir = source_name.rsplit_once('/').map(|x| x.0.to_string()).unwrap_or_default();
  let named = |s: &str| -> String {
    if s == name { return source_name.to_string(); }
    if let Some((_, inside)) = s.rsplit_once("node_modules/") { return format!("npm:{}", inside); }
    let abs = out_dir.join(s);
    let abs = abs.canonicalize().unwrap_or(abs);
    for m in &modules {
      if m.path.canonicalize().ok().as_ref() == Some(&abs) {
        return format!("{}.{}", m.specifier, m.src.extension());
      }
    }
    match abs.strip_prefix(&dir) {
      Ok(rel) => format!("{}/{}", shown_dir, rel.display()),
      Err(_) => abs.display().to_string(),
    }
  };
  split_bundle_naming(&text, source_name, key, &named).map(Some).map_err(failed)
}
