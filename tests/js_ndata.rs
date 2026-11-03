//! JavaScript commands get live ndata handles, as Python commands do
//! (python_ndata_protocols.rs): an object or array argument is the caller's
//! own ndata - changes are seen by the caller - and behaves like a plain
//! object or array. Writing a plain value stores new ndata; writing a handle
//! stores the same object; bytes cross as Uint8Array.
#![cfg(feature = "javascript_runtime")]

use flowlang::jscmd::{run, JsCall};
use ndata::data::Data;
use ndata::dataarray::DataArray;
use ndata::databytes::DataBytes;
use ndata::dataobject::DataObject;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Once;

static INIT: Once = Once::new();
static SEQ: AtomicUsize = AtomicUsize::new(0);

fn init() { INIT.call_once(|| { flowlang::init("data"); }); }

fn call(params: &[&str], body: &str, args: DataObject) -> Result<DataObject, String> {
  run(&JsCall {
    label: format!("jsnd{}:ctl:cmd{}", std::process::id(), SEQ.fetch_add(1, Ordering::SeqCst)),
    params: params.iter().map(|s| s.to_string()).collect(),
    body: body.to_string(),
    returntype: "Any".to_string(),
    args,
    timeout: None,
    module: None,
  })
}

fn json(s: &str) -> serde_json::Value { serde_json::from_str(s).unwrap() }
fn value(env: &DataObject) -> serde_json::Value {
  let mut o = DataObject::new();
  o.set_property("v", env.get_property("a"));
  serde_json::from_str::<serde_json::Value>(&o.to_string()).unwrap()["v"].clone()
}
fn obj(s: &str) -> DataObject { init(); DataObject::from_string(s) }

#[test]
fn an_object_argument_is_the_callers_object() {
  let o = obj(r#"{"a":1,"gone":true}"#);
  let mut args = obj("{}");
  args.put_object("o", o.clone());
  let r = call(&["o"], "o.a += 1;\no.b = 'new';\ndelete o.gone;\nreturn null;", args).unwrap();
  assert_eq!(value(&r), json("null"));
  assert_eq!(json(&o.to_string()), json(r#"{"a":2,"b":"new"}"#));
}

#[test]
fn an_array_argument_is_the_callers_array_and_an_array() {
  init();
  let a = DataArray::from_string("[3,1,2]");
  let mut args = obj("{}");
  args.put_array("xs", a.clone());
  let body = "xs.push(4);\nxs.sort((p, q) => p - q);\nxs[5] = 9;\nconst sum = xs.reduce((s, x) => s + (x ?? 0), 0);\nreturn [Array.isArray(xs), xs.length, xs.includes(4), sum, [...xs].length];";
  let r = call(&["xs"], body, args).unwrap();
  assert_eq!(value(&r), json("[true,6,true,19,6]"));
  assert_eq!(json(&a.to_string()), json("[1,2,3,4,null,9]"));
  let mut args = obj("{}");
  args.put_array("xs", a.clone());
  call(&["xs"], "xs.length = 2;\nxs.splice(1, 0, 'mid');\nreturn null", args).unwrap();
  assert_eq!(json(&a.to_string()), json(r#"[1,"mid",2]"#));
}

#[test]
fn nested_values_are_handles_on_the_same_ndata() {
  let o = obj(r#"{"inner":{"n":1}}"#);
  let inner = o.get_object("inner");
  let mut args = obj("{}");
  args.put_object("o", o.clone());
  let r = call(&["o"], "const i = o.inner;\ni.n = 5;\nreturn i;", args).unwrap();
  assert_eq!(inner.get_int("n"), 5);
  // Returning a handle returns the same object, not a copy.
  assert_eq!(r.get_object("a").data_ref, inner.data_ref);
}

#[test]
fn handles_behave_like_plain_objects() {
  let o = obj(r#"{"a":1,"b":[1,2],"c":{"d":null}}"#);
  let mut args = obj("{}");
  args.put_object("o", o.clone());
  let body = "const keys = [];\nfor (const k in o) keys.push(k);\nreturn {\n  keys: Object.keys(o).sort(), forin: keys.sort(), entries: Object.entries(o).length,\n  json: JSON.parse(JSON.stringify(o)), spread: { ...o, e: 1 }, has: ['a' in o, 'z' in o],\n  handle: ndata.isHandle(o), plain: ndata.isHandle(ndata.toJS(o)), tag: Object.prototype.toString.call(o),\n  isObject: o instanceof ndata.NDataObject, missing: o.zzz === undefined, nested: o.c.d,\n};";
  let r = call(&["o"], body, args).unwrap();
  assert_eq!(value(&r), json(r#"{"keys":["a","b","c"],"forin":["a","b","c"],"entries":3,
    "json":{"a":1,"b":[1,2],"c":{"d":null}},"spread":{"a":1,"b":[1,2],"c":{"d":null},"e":1},"has":[true,false],
    "handle":true,"plain":false,"tag":"[object NDataObject]","isObject":true,"missing":true,"nested":null}"#));
}

#[test]
fn writes_store_plain_values_as_new_ndata_and_handles_as_themselves() {
  let a = obj("{}");
  let b = obj(r#"{"z":0}"#);
  let mut args = obj("{}");
  args.put_object("a", a.clone());
  args.put_object("b", b.clone());
  let body = "a.child = { x: [1, { y: 2 }], u: undefined, when: new Date(0) };\na.ref = b;\nb.z = 1;\na.nums = [3, 1.5, NaN, 2 ** 60];\nreturn a.ref.z;";
  let r = call(&["a", "b"], body, args).unwrap();
  assert_eq!(value(&r), json("1"));
  assert_eq!(a.get_object("ref").data_ref, b.data_ref);
  assert_eq!(json(&a.get_object("child").to_string()), json(r#"{"x":[1,{"y":2}],"when":"1970-01-01T00:00:00.000Z"}"#));
  let nums = a.get_array("nums");
  assert!(matches!(nums.get_property(0), Data::DInt(3)));
  assert!(matches!(nums.get_property(1), Data::DFloat(f) if f == 1.5));
  assert!(matches!(nums.get_property(2), Data::DNull));
  assert!(matches!(nums.get_property(3), Data::DFloat(_)));
}

#[test]
fn bytes_cross_as_uint8array() {
  init();
  let mut args = obj("{}");
  args.put_bytes("data", DataBytes::from_bytes(&vec![1, 2, 3]));
  let o = obj("{}");
  args.put_object("o", o.clone());
  let r = call(&["data", "o"], "o.bin = new Uint8Array([7, 8]);\nreturn new Uint8Array([data instanceof Uint8Array ? 1 : 0, data.length, data[2]]);", args).unwrap();
  assert_eq!(r.get_bytes("a").get_data(), vec![1, 3, 3]);
  assert_eq!(o.get_bytes("bin").get_data(), vec![7, 8]);
}

#[test]
fn what_cannot_be_stored_says_why() {
  for (body, why) in [
    ("o.m = new Map();", "a Map"),
    ("o.f = () => 1;", "a function"),
    ("o.n = 10n;", "a BigInt"),
    ("const c = {}; c.self = c; o.c = c;", "a cyclic structure"),
    ("o.p = Promise.resolve(1);", "a Promise"),
  ] {
    let mut args = obj("{}");
    args.put_object("o", obj("{}"));
    let e = call(&["o"], body, args).unwrap_err();
    assert!(e.contains("TypeError") && e.contains(why), "{}: {}", body, e);
  }
  let e = call(&[], "return { inner: new Set([1]) };", obj("{}")).unwrap_err();
  assert!(e.contains("could not be converted: a Set"), "{}", e);
}

#[test]
fn new_ndata_can_be_made_in_javascript() {
  let r = call(&[], "const o = ndata.object();\no.list = ndata.array();\no.list.push(1, 2);\nreturn o;", obj("{}")).unwrap();
  assert_eq!(value(&r), json(r#"{"list":[1,2]}"#));
}
