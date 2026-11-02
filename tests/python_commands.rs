//! Python commands: the result envelope, the value conversion rules and the
//! error reports. Every test here runs in BOTH execution modes:
//!
//!   cargo test --test python_commands                              # subprocess (python3)
//!   cargo test --test python_commands --features=python_runtime    # embedded (PyO3)
//!
//! A Python command must look exactly like a Rust command to its callers:
//! `{"a": value}` on success, `{"a": {"status": "err", "msg"}}` on failure.
//! Before this contract, a Python command returned `{"status", "msg"|"data"}`
//! (or merged a returned dict into the top level), so a flow's command op
//! faulted with "the callee's result lacks 'a'", and errors used
//! `status: "error"`, which callers checking for "err" read as success.

use flowlang::case::Case;
use flowlang::code::Code;
use flowlang::command::Command;
use flowlang::pycmd::{run, PyCall};
use ndata::data::Data;
use ndata::dataobject::DataObject;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Once;

static INIT: Once = Once::new();
static SEQ: AtomicUsize = AtomicUsize::new(0);

fn init() {
  INIT.call_once(|| { flowlang::init("data"); });
}

const EMBEDDED: bool = cfg!(feature = "python_runtime");

/// Writes a command module laid out exactly as `flowb` generates it
/// (imports, `execute`, then the user's body indented under `def <func>`),
/// in a fresh package so the interpreter's module cache never interferes.
fn command(imports: &str, params: &[&str], body: &str, returntype: &str, args: DataObject) -> PyCall {
  let n = SEQ.fetch_add(1, Ordering::SeqCst);
  command_in(&format!("pytest{}_{}", std::process::id(), n), imports, params, body, returntype, args)
}

fn command_in(lib: &str, imports: &str, params: &[&str], body: &str, returntype: &str, args: DataObject) -> PyCall {
  init();
  let base = std::env::temp_dir().join("flowlang_python_commands");
  let dir = base.join(lib).join("ctl");
  std::fs::create_dir_all(&dir).unwrap();
  let func = "cmd";
  let mut src = String::new();
  src.push_str(imports);
  src.push_str("\n\n");
  let extract: Vec<String> = params.iter().map(|p| format!("args['{}']", p)).collect();
  src.push_str(&format!("def execute(args):\n    return {}({})\n\n", func, extract.join(", ")));
  src.push_str(&format!("def {}({}):\n", func, params.join(", ")));
  for line in body.lines() { src.push_str(&format!("    {}\n", line)); }
  src.push_str("\n\nif __name__ == \"__main__\":\n    import sys\n    import json\n    print(json.dumps(execute(json.loads(sys.argv[1]))))\n");
  let file: PathBuf = dir.join(format!("{}.py", func));
  std::fs::write(&file, src).unwrap();
  PyCall {
    label: format!("{}:ctl:{}", lib, func),
    base_dir: base,
    module: format!("{}.ctl.{}", lib, func),
    file,
    func: func.to_string(),
    returntype: returntype.to_string(),
    args,
  }
}

fn ret(body: &str) -> Result<DataObject, String> {
  init();
  run(&command("", &[], body, "Any", DataObject::new()))
}
fn ret_imports(imports: &str, body: &str) -> Result<DataObject, String> {
  init();
  run(&command(imports, &[], body, "Any", DataObject::new()))
}
/// The envelope's value as `{"v": ...}`. Read from the envelope while it is
/// alive: the envelope owns the value.
fn json(env: &DataObject) -> String {
  let mut o = DataObject::new();
  o.set_property("v", env.get_property("a"));
  o.to_string()
}

// ---- conversion: exact, or an error -----------------------------------------------------

#[test]
fn list_of_small_ints_is_a_list_not_bytes() {
  // Was "01 02 03": a list of ints 0..=255 extracted as Vec<u8>.
  assert_eq!(json(&ret("return [1, 2, 3]").unwrap()), r#"{"v":[1,2,3]}"#);
  assert_eq!(json(&ret("return {'scores': [10, 20, 30]}").unwrap()), r#"{"v":{"scores":[10,20,30]}}"#);
}

#[test]
fn tuple_is_an_array() {
  // Was "01 02".
  assert_eq!(json(&ret("return (1, 2)").unwrap()), r#"{"v":[1,2]}"#);
  assert_eq!(json(&ret("return {'t': ('x', 2, None)}").unwrap()), r#"{"v":{"t":["x",2,null]}}"#);
}

#[test]
fn scalars_keep_their_types() {
  assert_eq!(json(&ret("return True").unwrap()), r#"{"v":true}"#);
  assert_eq!(json(&ret("return 0").unwrap()), r#"{"v":0}"#);
  assert_eq!(json(&ret("return 1.5").unwrap()), r#"{"v":1.5}"#);
  assert_eq!(json(&ret("return 'hi'").unwrap()), r#"{"v":"hi"}"#);
  assert_eq!(json(&ret("return None").unwrap()), r#"{"v":null}"#);
  assert_eq!(json(&ret(&format!("return {}", i64::MAX)).unwrap()), format!(r#"{{"v":{}}}"#, i64::MAX));
  assert_eq!(json(&ret(&format!("return {}", i64::MIN)).unwrap()), format!(r#"{{"v":{}}}"#, i64::MIN));
}

#[test]
fn int_and_str_subclasses_and_numeric_abcs_convert() {
  assert_eq!(json(&ret_imports("import enum", "class E(enum.IntEnum):\n    X = 7\nreturn E.X").unwrap()), r#"{"v":7}"#);
  assert_eq!(json(&ret_imports("import fractions", "return fractions.Fraction(1, 4)").unwrap()), r#"{"v":0.25}"#);
}

#[test]
fn bytes_and_bytearray_are_bytes() {
  let d = ret("return b'Hi'").unwrap();
  let d2 = ret("return bytearray(b'Hi')").unwrap();
  if EMBEDDED {
    assert!(matches!(d.get_property("a"), Data::DBytes(_)), "bytes -> DBytes, got {:?}", d.get_property("a"));
    assert!(matches!(d2.get_property("a"), Data::DBytes(_)), "bytearray -> DBytes, got {:?}", d2.get_property("a"));
  } else {
    // By value over JSON: ndata's JSON form of DBytes.
    assert_eq!(json(&d), r#"{"v":"48 69"}"#);
    assert_eq!(json(&d2), r#"{"v":"48 69"}"#);
  }
}

#[test]
fn int_outside_i64_is_an_overflow_error_not_a_float() {
  // Was 9.223372036854776e+18.
  let e = ret("return 2**63").unwrap_err();
  assert!(e.starts_with("OverflowError: "), "{}", e);
  assert!(e.contains("9223372036854775808"), "{}", e);
  let e = ret("return {'big': -(2**70)}").unwrap_err();
  assert!(e.starts_with("OverflowError: "), "{}", e);
}

#[test]
fn unsupported_types_are_type_errors_with_a_hint() {
  // Decimal was silently rounded to a float.
  let e = ret_imports("import decimal", "return decimal.Decimal('1.10')").unwrap_err();
  assert!(e.starts_with("TypeError: ") && e.contains("Decimal") && e.contains("str(x)"), "{}", e);
  let e = ret("return {1, 2}").unwrap_err();
  assert!(e.starts_with("TypeError: ") && e.contains("set") && e.contains("sorted(x)"), "{}", e);
  let e = ret_imports("import datetime", "return datetime.date(2026, 1, 2)").unwrap_err();
  assert!(e.starts_with("TypeError: ") && e.contains("isoformat"), "{}", e);
  let e = ret("return {1: 'a'}").unwrap_err();
  assert!(e.starts_with("TypeError: ") && e.contains("dict keys must be str"), "{}", e);
}

#[test]
fn flat_requires_a_dict() {
  init();
  let ok = run(&command("", &[], "return {'status': 'ok', 'x': 1}", "FLAT", DataObject::new())).unwrap();
  assert_eq!(json(&ok), r#"{"v":{"status":"ok","x":1}}"#);
  let e = run(&command("", &[], "return [1]", "FLAT", DataObject::new())).unwrap_err();
  assert!(e.starts_with("TypeError: a FLAT command must return a dict or NDataObject, got list"), "{}", e);
}

// ---- arguments ---------------------------------------------------------------------------

#[test]
fn arguments_arrive_by_name() {
  init();
  let mut args = DataObject::new();
  args.put_string("name", "world");
  args.put_int("n", 3);
  let mut o = DataObject::new();
  o.put_string("k", "v");
  args.put_object("obj", o);
  let call = command("", &["name", "n", "obj"], "return [name, n * 2, obj['k']]", "JSONArray", args);
  assert_eq!(json(&run(&call).unwrap()), r#"{"v":["world",6,"v"]}"#);
}

#[test]
fn print_does_not_touch_the_result() {
  let d = ret("print('noise on stdout')\nimport sys\nsys.stdout.write('more\\n')\nreturn 'clean'").unwrap();
  assert_eq!(json(&d), r#"{"v":"clean"}"#);
}

// ---- error reports -----------------------------------------------------------------------

#[test]
fn exception_reports_body_line_and_traceback() {
  init();
  let call = command("import math", &[], "x = 1\ndef inner():\n    return 1 / 0\nreturn inner()", "Any", DataObject::new());
  let e = run(&call).unwrap_err();
  let head = e.lines().next().unwrap();
  assert_eq!(head, format!("ZeroDivisionError: division by zero ({}, body line 3)", call.label));
  assert!(e.contains("Traceback (most recent call last):"), "{}", e);
  assert!(e.contains("[body line 4]"), "the calling frame is annotated too: {}", e);
  assert!(e.contains("[body line 3]"), "{}", e);
}

#[test]
fn syntax_error_reports_body_line() {
  init();
  let call = command("", &[], "x = 1\nreturn (", "Any", DataObject::new());
  let e = run(&call).unwrap_err();
  let head = e.lines().next().unwrap();
  assert!(head.starts_with("SyntaxError: "), "{}", e);
  assert!(head.ends_with(&format!("({}, body line 2)", call.label)), "{}", e);
}

#[test]
fn import_error_reports_imports_line() {
  init();
  let call = command("import json\nimport no_such_module_xyz", &[], "return 1", "Any", DataObject::new());
  let e = run(&call).unwrap_err();
  let head = e.lines().next().unwrap();
  assert!(head.starts_with("ModuleNotFoundError: No module named 'no_such_module_xyz'"), "{}", e);
  assert!(head.ends_with(&format!("({}, imports line 2)", call.label)), "{}", e);
}

// ---- the envelope, through Command and through a flow ------------------------------------

#[test]
fn command_result_uses_the_rust_envelope() {
  init();
  let mut args = DataObject::new();
  args.put_string("a", "world");
  let r = Command::lookup("testflow", "testflow", "test_python").execute(args).unwrap();
  assert_eq!(r.to_string(), r#"{"a":"Hello world from python"}"#);
}

#[test]
fn missing_parameter_is_an_err_envelope() {
  init();
  let r = Command::lookup("testflow", "testflow", "test_python").execute(DataObject::new()).unwrap();
  let a = r.get_object("a");
  assert_eq!(a.get_string("status"), "err");
  assert_eq!(a.get_string("msg"), "missing required parameter: a");
}

#[test]
fn python_command_runs_as_a_flow_op() {
  // Was a fault: "the callee's result lacks 'a'".
  init();
  let op = r#"{"cmd":"testflow:zplvxj1802d5739aco1a:ovgzjp1822699e1d1w19","in":{"a":{"mode":"regular","type":"Any","x":0}},"name":"test_python","out":{"r":{"mode":"regular","type":"Any","x":0}},"pos":{"x":0,"y":0,"z":0},"type":"command","width":1.5}"#;
  let body = format!(
    r#"{{"cmds":[{}],"cons":[{{"dest":[0,"a"],"src":[-1,"v"]}},{{"dest":[-2,"r"],"src":[0,"r"]}}],"input":{{"v":{{"mode":"in","type":"Any","x":0}}}},"output":{{"r":{{"mode":"out","type":"Any","x":0}}}}}}"#,
    op
  );
  let mut code = Code::new(Case::new(&body));
  let out = code.execute(DataObject::from_string(r#"{"v":"flow"}"#)).unwrap();
  assert_eq!(out.get_string("r"), "Hello flow from python");
}

// ---- sys.path ----------------------------------------------------------------------------

#[test]
fn sys_path_has_no_cwd_or_command_dir() {
  // Was: the server's cwd and each command's own directory inserted at the
  // FRONT of sys.path, so a stray json.py in either shadowed the stdlib.
  init();
  let call = command("import sys", &[], "return [p for p in sys.path if isinstance(p, str)]", "Any", DataObject::new());
  let d = run(&call).unwrap();
  let paths: Vec<String> = d.get_array("a").objects().iter().map(|p| p.string()).collect();
  let cwd = std::env::current_dir().unwrap().display().to_string();
  let cmd_dir = call.file.parent().unwrap().display().to_string();
  for bad in ["", ".", cwd.as_str(), cmd_dir.as_str()] {
    assert!(!paths.iter().any(|p| p == bad), "sys.path contains {:?}: {:?}", bad, paths);
  }
  let base = call.base_dir.display().to_string();
  assert!(paths.contains(&base), "sys.path lacks the package root {}: {:?}", base, paths);
}

#[test]
fn library_named_like_a_stdlib_module() {
  // A library named `colorsys` shadowed the stdlib module for EVERY command
  // while package roots went first on sys.path. Now the stdlib wins, and only
  // that library's own commands fail - with an error that says why.
  init();
  let call = command_in("colorsys", "", &[], "return 1", "Any", DataObject::new());
  let e = run(&call).unwrap_err();
  let head = e.lines().next().unwrap();
  assert!(head.starts_with("ImportError: library 'colorsys' cannot be imported as a Python package: that name is taken by the Python module at "), "{}", e);
  let other = ret("import colorsys\nreturn list(colorsys.rgb_to_hsv(1.0, 0.0, 0.0))").unwrap();
  assert_eq!(json(&other), r#"{"v":[0.0,1.0,1.0]}"#);
}

// ---- the ndata_py and flowlang modules -------------------------------------------------

#[test]
fn ndata_py_builds_objects() {
  // Was: ModuleNotFoundError - the #[pymodule] was never registered.
  let r = ret("import ndata_py\no = ndata_py.NDataObject()\no['x'] = 1\na = ndata_py.NDataArray()\na.append('y')\no['arr'] = a\nreturn o");
  if EMBEDDED {
    assert_eq!(json(&r.unwrap()), r#"{"v":{"arr":["y"],"x":1}}"#);
  } else {
    let e = r.unwrap_err();
    assert!(e.starts_with("ModuleNotFoundError: No module named 'ndata_py'"), "{}", e);
  }
}

fn call_body(expr: &str) -> String {
  format!("import flowlang\ntry:\n    return {}\nexcept Exception as e:\n    return '%s: %s' % (type(e).__name__, e)", expr)
}

#[test]
fn flowlang_call_runs_every_kind_of_command() {
  let cases = [
    // Python -> Rust -> Python: the lock is released and re-taken.
    ("flowlang.call('testflow', 'testflow', 'test_python', a='nested')", r#""Hello nested from python""#),
    ("flowlang.call('testflow', 'testflow', 'test_rust', a='x')", r#""Hello x from rust""#),
    // a flow with one output returns that value
    ("flowlang.call('testflow', 'testflow', 'test_add', a=300, b=120)", r#""300+120=420""#),
    // a flow with several outputs returns them all
    ("flowlang.call('testflow', 'testflow', 'test_loop', a=3)", r#"{"a":4,"b":[4]}"#),
    // a declared type is cast, as for an HTTP call
    ("flowlang.call('testflow', 'testflow', 'test_add', a='2', b=3)", r#""2+3=5""#),
  ];
  for (expr, want) in cases {
    let r = ret(&call_body(expr));
    if EMBEDDED {
      assert_eq!(json(&r.unwrap()), format!(r#"{{"v":{}}}"#, want), "{}", expr);
    } else {
      let got = json(&r.unwrap());
      assert!(got.contains("RuntimeError: flowlang.call(") && got.contains("--features=python_runtime"), "{}", got);
    }
  }
}

#[test]
fn flowlang_call_errors() {
  if !EMBEDDED { return; } // the subprocess stub raises RuntimeError for every call (above)
  let cases = [
    ("flowlang.call('testflow', 'testflow', 'test_python')",
     "CommandError: testflow:testflow:test_python: missing required parameter: a"),
    ("flowlang.call('testflow', 'testflow', 'no_such_cmd')",
     "LookupError: no command testflow:testflow:no_such_cmd: No such command testflow:testflow:no_such_cmd"),
    ("flowlang.call('testflow', 'testflow', 'test_add', a='abc', b=1)",
     "TypeError: arguments for testflow:testflow:test_add do not match its declared parameter types: "),
    ("flowlang.call('testflow', 'testflow', 'test_add', a={1, 2}, b=1)",
     "TypeError: argument a: cannot convert Python set to ndata"),
  ];
  for (expr, want) in cases {
    let got = ret(&call_body(expr)).unwrap().get_string("a");
    assert!(got.starts_with(want), "{}\n  got:  {}\n  want: {}", expr, got, want);
  }
  // CommandError is a real exception class, importable by name.
  let r = ret("from flowlang import CommandError\nreturn issubclass(CommandError, Exception)").unwrap();
  assert_eq!(json(&r), r#"{"v":true}"#);
}

#[test]
fn library_named_flowlang_is_reported_as_taken() {
  if !EMBEDDED { return; }
  let call = command_in("flowlang", "", &[], "return 1", "Any", DataObject::new());
  let e = run(&call).unwrap_err();
  assert!(e.starts_with("ImportError: library 'flowlang' cannot be imported as a Python package: that name is taken by the built-in module 'flowlang'"), "{}", e);
}

#[test]
fn returned_containers_survive_a_concurrent_gc() {
  // A container built by the conversion (or by Python via ndata_py) was
  // handed back as a bare reference after its last handle was dropped; a GC
  // on another thread in between freed it ("Heap::incr: Invalid index").
  init();
  use std::sync::atomic::AtomicBool;
  use std::sync::Arc;
  let stop = Arc::new(AtomicBool::new(false));
  let s2 = stop.clone();
  let gc = std::thread::spawn(move || {
    while !s2.load(Ordering::SeqCst) { flowlang::datastore::DataStore::gc(); }
  });
  let dict_cmd = command("", &[], "return {'k': [1, {'z': 3}], 'b': b'x'}", "Any", DataObject::new());
  let nd_body = if EMBEDDED {
    // built in Python, stored in an array, popped back out, returned
    "import ndata_py\no = ndata_py.NDataObject()\no['k'] = [1, {'z': 3}]\na = ndata_py.NDataArray()\na.append(o)\ndel o\nreturn a.pop()"
  } else {
    "return {'k': [1, {'z': 3}]}"
  };
  let nd_cmd = command("", &[], nd_body, "Any", DataObject::new());
  let rounds = if EMBEDDED { 300 } else { 20 }; // a subprocess per call is slow
  for _ in 0..rounds {
    let d = run(&dict_cmd).unwrap();
    assert_eq!(d.get_object("a").get_array("k").get_object(1).get_int("z"), 3);
    let d = run(&nd_cmd).unwrap();
    assert_eq!(json(&d), r#"{"v":{"k":[1,{"z":3}]}}"#);
  }
  stop.store(true, Ordering::SeqCst);
  gc.join().unwrap();
}

#[test]
fn ndata_array_pop() {
  if !EMBEDDED { return; }
  let r = ret("import ndata_py\na = ndata_py.NDataArray()\na.append({'k': 1})\na.append('s')\nlast = a.pop()\nfirst = a.pop(0)\nreturn [len(a), last, first['k']]").unwrap();
  assert_eq!(json(&r), r#"{"v":[0,"s",1]}"#);
  let e = ret("import ndata_py\nreturn ndata_py.NDataArray().pop()").unwrap_err();
  assert!(e.starts_with("IndexError: pop from empty list"), "{}", e);
}
