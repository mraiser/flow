//! Live ndata handles for JavaScript commands - the counterpart of Python's
//! `NDataObject` / `NDataArray` (`pywrapper.rs`): a JavaScript handle holds a
//! `DataObject` or `DataArray` handle, so reading and writing it reads and
//! writes the same ndata the Rust and Python commands see, with no copying.
//! `jsndata.js` puts a `Proxy` in front of each handle so it behaves like a
//! plain object or array.
//!
//! V8 owns a handle's lifetime: `Nd` is a garbage-collected (cppgc) object,
//! and when V8 collects it the ndata handle inside is dropped, which queues
//! its reference for ndata's GC - exactly as when a Rust handle goes out of
//! scope. The ops take `Nd` through `#[cppgc]`, so deno_core does the
//! unwrapping.
//!
//! Values: a key or index reads as its kind (`op_nd_kind`) and then its
//! value through the getter for that kind. Containers come back as new
//! handles on the same ndata object; bytes as a copy (`Uint8Array`). Numbers
//! are JavaScript numbers: an integral value within 2^53 is written as an
//! ndata Integer, anything else as a Float.

use std::cell::RefCell;
use std::collections::HashMap;

use deno_core::op2;
use deno_core::v8;
use deno_core::GarbageCollected;
use ndata::data::Data;
use ndata::dataarray::DataArray;
use ndata::databytes::DataBytes;
use ndata::dataobject::DataObject;

use crate::pycmd::Value;

pub(crate) const NDATA_JS: &str = include_str!("jsndata.js");

/// A JavaScript handle on an ndata object or array.
pub(crate) enum Nd {
  Obj(DataObject),
  Arr(DataArray),
}

// SAFETY: the trait's contract is that `trace` visits every V8 reference
// the object holds. `Nd` holds none - only an ndata handle - so there is
// nothing to visit.
unsafe impl GarbageCollected for Nd {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"NData"
  }
}

impl Nd {
  /// The value at `key` (an index, for an array), if any.
  fn get(&self, key: &str) -> Option<Data> {
    match self {
      Nd::Obj(o) => o.try_get_property(key).ok(),
      Nd::Arr(a) => key.parse::<usize>().ok().filter(|i| *i < a.len()).map(|i| a.get_property(i)),
    }
  }

  /// Stores a value; for an array, an index at the end appends and one past
  /// it pads with nulls first.
  fn put(&self, key: &str, v: Value) {
    match self {
      Nd::Obj(o) => {
        let mut o = o.clone();
        v.put(&mut o, key);
      }
      Nd::Arr(a) => {
        let Ok(i) = key.parse::<usize>() else { return };
        let mut a = a.clone();
        while a.len() < i { a.push_null(); }
        if i == a.len() { v.push(&mut a); } else { v.put_at(&mut a, i); }
      }
    }
  }
}

/// What `op_nd_kind` answers.
const MISSING: u32 = 0;
const NULL: u32 = 1;
const BOOLEAN: u32 = 2;
const INTEGER: u32 = 3;
const FLOAT: u32 = 4;
const STRING: u32 = 5;
const OBJECT: u32 = 6;
const ARRAY: u32 = 7;
const BYTES: u32 = 8;

#[op2]
#[cppgc]
pub(crate) fn op_nd_new(is_array: bool) -> Nd {
  if is_array { Nd::Arr(DataArray::new()) } else { Nd::Obj(DataObject::new()) }
}

#[op2(fast)]
pub(crate) fn op_nd_is_array(#[cppgc] n: &Nd) -> bool {
  matches!(n, Nd::Arr(_))
}

#[op2(fast)]
pub(crate) fn op_nd_len(#[cppgc] n: &Nd) -> u32 {
  match n {
    Nd::Obj(o) => o.get_keys().len() as u32,
    Nd::Arr(a) => a.len() as u32,
  }
}

/// An object's keys (an array's are its indices: use its length).
#[op2]
#[serde]
pub(crate) fn op_nd_keys(#[cppgc] n: &Nd) -> Vec<String> {
  match n {
    Nd::Obj(o) => o.get_keys(),
    Nd::Arr(a) => (0..a.len()).map(|i| i.to_string()).collect(),
  }
}

#[op2(fast)]
pub(crate) fn op_nd_kind(#[cppgc] n: &Nd, #[string] key: &str) -> u32 {
  match n.get(key) {
    None => MISSING,
    Some(Data::DNull) => NULL,
    Some(Data::DBoolean(_)) => BOOLEAN,
    Some(Data::DInt(_)) => INTEGER,
    Some(Data::DFloat(_)) => FLOAT,
    Some(Data::DString(_)) => STRING,
    Some(Data::DObject(_)) => OBJECT,
    Some(Data::DArray(_)) => ARRAY,
    Some(Data::DBytes(_)) => BYTES,
  }
}

#[op2]
#[string]
pub(crate) fn op_nd_get_str(#[cppgc] n: &Nd, #[string] key: &str) -> String {
  match n.get(key) { Some(Data::DString(s)) => s, _ => String::new() }
}

#[op2(fast)]
pub(crate) fn op_nd_get_num(#[cppgc] n: &Nd, #[string] key: &str) -> f64 {
  match n.get(key) {
    Some(Data::DInt(i)) => i as f64,
    Some(Data::DFloat(f)) => f,
    _ => f64::NAN,
  }
}

#[op2(fast)]
pub(crate) fn op_nd_get_bool(#[cppgc] n: &Nd, #[string] key: &str) -> bool {
  matches!(n.get(key), Some(Data::DBoolean(true)))
}

/// A new handle on the object or array at `key` - the same ndata.
#[op2]
#[cppgc]
pub(crate) fn op_nd_get_nd(#[cppgc] n: &Nd, #[string] key: &str) -> Nd {
  match n.get(key) {
    Some(d @ Data::DObject(_)) => Nd::Obj(d.object()),
    Some(d @ Data::DArray(_)) => Nd::Arr(d.array()),
    _ => Nd::Obj(DataObject::new()),
  }
}

/// A copy of the bytes at `key`.
#[op2]
#[buffer]
pub(crate) fn op_nd_get_bytes(#[cppgc] n: &Nd, #[string] key: &str) -> Vec<u8> {
  match n.get(key) {
    Some(d @ Data::DBytes(_)) => d.bytes().get_data(),
    _ => Vec::new(),
  }
}

#[op2(fast)]
pub(crate) fn op_nd_set_str(#[cppgc] n: &Nd, #[string] key: &str, #[string] v: &str) {
  n.put(key, Value::Scalar(Data::DString(v.to_string())));
}

/// An integral number within 2^53 is stored as an Integer, any other as a
/// Float (NaN and the infinities as null, as JSON has them).
#[op2(fast)]
pub(crate) fn op_nd_set_num(#[cppgc] n: &Nd, #[string] key: &str, v: f64) {
  let d = if !v.is_finite() {
    Data::DNull
  } else if v.fract() == 0.0 && v.abs() <= 9_007_199_254_740_992.0 {
    Data::DInt(v as i64)
  } else {
    Data::DFloat(v)
  };
  n.put(key, Value::Scalar(d));
}

#[op2(fast)]
pub(crate) fn op_nd_set_bool(#[cppgc] n: &Nd, #[string] key: &str, v: bool) {
  n.put(key, Value::Scalar(Data::DBoolean(v)));
}

#[op2(fast)]
pub(crate) fn op_nd_set_null(#[cppgc] n: &Nd, #[string] key: &str) {
  n.put(key, Value::Scalar(Data::DNull));
}

/// Stores the object or array `child` holds - the same ndata, not a copy.
#[op2(fast)]
pub(crate) fn op_nd_set_nd(#[cppgc] n: &Nd, #[string] key: &str, #[cppgc] child: &Nd) {
  let v = match child {
    Nd::Obj(o) => Value::Object(o.clone()),
    Nd::Arr(a) => Value::Array(a.clone()),
  };
  n.put(key, v);
}

#[op2]
pub(crate) fn op_nd_set_bytes(#[cppgc] n: &Nd, #[string] key: &str, #[anybuffer] v: &[u8]) {
  n.put(key, Value::Bytes(DataBytes::from_bytes(&v.to_vec())));
}

/// Removes an object's key, or truncates an array to `len` when `key` is
/// "length" - the two ways a handle shrinks.
#[op2(fast)]
pub(crate) fn op_nd_delete(#[cppgc] n: &Nd, #[string] key: &str) {
  match n {
    Nd::Obj(o) => {
      let mut o = o.clone();
      o.remove_property(key);
    }
    Nd::Arr(a) => {
      // Deleting an element leaves a hole, as in JavaScript: null.
      if let Ok(i) = key.parse::<usize>() {
        if i < a.len() { a.clone().set_property(i, Data::DNull); }
      }
    }
  }
}

/// Sets an array's length: shorter drops the end, longer pads with nulls.
#[op2(fast)]
pub(crate) fn op_nd_set_len(#[cppgc] n: &Nd, len: u32) {
  if let Nd::Arr(a) = n {
    let mut a = a.clone();
    let len = len as usize;
    while a.len() > len { a.remove_property(a.len() - 1); }
    while a.len() < len { a.push_null(); }
  }
}

// ---- the values of a call ----------------------------------------------------------------

thread_local! {
  /// Calls in flight on the engine thread: their arguments (one per declared
  /// parameter, in order) and the array their result is written to.
  static CALLS: RefCell<HashMap<u32, (DataArray, DataArray)>> = RefCell::new(HashMap::new());
  static NEXT: std::cell::Cell<u32> = const { std::cell::Cell::new(1) };
}

/// Registers a call's arguments; returns its id and the array its result
/// will be written to (at index 0).
pub(crate) fn begin_call(args: DataArray) -> (u32, DataArray) {
  let id = NEXT.with(|n| { let id = n.get(); n.set(id.wrapping_add(1).max(1)); id });
  let out = DataArray::new();
  CALLS.with(|c| { c.borrow_mut().insert(id, (args, out.clone())); });
  (id, out)
}

/// Forgets a call (answered, failed or abandoned).
pub(crate) fn end_call(id: u32) {
  CALLS.with(|c| { c.borrow_mut().remove(&id); });
}

/// Forgets every call (the engine is restarting).
pub(crate) fn end_all_calls() {
  CALLS.with(|c| c.borrow_mut().clear());
}

/// The result written to a call's `out` array: the value, if one was
/// written.
pub(crate) fn result_of(out: &DataArray) -> Option<Value> {
  if out.len() == 0 { None } else { Some(Value::from_data(out.get_property(0))) }
}

#[op2]
#[cppgc]
pub(crate) fn op_flowlang_call_args(#[smi] id: u32) -> Nd {
  CALLS.with(|c| c.borrow().get(&id).map(|(a, _)| Nd::Arr(a.clone()))).unwrap_or_else(|| Nd::Arr(DataArray::new()))
}

#[op2]
#[cppgc]
pub(crate) fn op_flowlang_call_out(#[smi] id: u32) -> Nd {
  CALLS.with(|c| c.borrow().get(&id).map(|(_, o)| Nd::Arr(o.clone()))).unwrap_or_else(|| Nd::Arr(DataArray::new()))
}
