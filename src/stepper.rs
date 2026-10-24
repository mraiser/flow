//! The flow stepper: the single source of truth for flow execution (Flow step debugger P6).
//!
//! A ready-set scheduler over a FRAME STACK, one event per step. `step_stack` advances a frame
//! stack one step; `run` continues on the default pick (flowlang's sweep order) to the end, which
//! is exactly `Code::execute`. The interpreter (`code.rs`) is a thin wrapper over this, so the
//! debugger and the runtime share one execution semantics (spec: docs/flow-language.md 1.5.4).
//!
//! Execution state lives in ndata frames (see `new_frame`): a frame is a case of a flow being run,
//! with the fired/done flags, the delivered inputs, the per-op results, the shared `out`, the
//! current case index, a cursor for the sweep, and the op in progress. The graphs are never
//! mutated. A `next` in the last case (I-4) and a stuck case (I-1/I-2) end the run with an error
//! event instead of panicking or hanging, so `Code::execute` reports both as `CodeException::Fail`.

use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use ndata::data::Data;
use std::panic;
use crate::primitives::Primitive;
use crate::command::Command;
use crate::datastore::DataStore;

// gc_each_eval was a per-evaluation DataObject/DataArray/DataBytes gc; on the continue hot path
// that is far too expensive. gc is owned by the Command boundary (command.rs), as before P6.

pub type Fault = (String, String);

fn fault(code: &str, msg: String) -> Fault { (code.to_string(), msg) }
fn panic_text(p: &Box<dyn std::any::Any + Send>) -> String {
  if let Some(s) = p.downcast_ref::<&str>() { s.to_string() }
  else if let Some(s) = p.downcast_ref::<String>() { s.clone() }
  else { "unknown panic".to_string() }
}
fn put_copy(o: &mut DataObject, k: &str, d: Data) {
  if d.is_object() { o.put_object(k, d.object().deep_copy()); }
  else if d.is_array() { o.put_array(k, d.array().deep_copy()); }
  else { o.set_property(k, d); }
}
pub fn is_obj(o: &DataObject, k: &str) -> bool { o.has(k) && o.get_property(k).is_object() }
pub fn is_arr(o: &DataObject, k: &str) -> bool { o.has(k) && o.get_property(k).is_array() }
fn shallow_obj(o: &DataObject) -> DataObject { let mut n = DataObject::new(); for (k, v) in o.objects() { n.set_property(&k, v); } n }
fn shallow_arr(a: &DataArray) -> DataArray { let mut n = DataArray::new(); for v in a.objects() { n.push_property(v); } n }
fn strs(a: &DataArray) -> Vec<String> { a.objects().iter().map(|d| d.string()).collect() }
fn vec_arr(v: &Vec<String>) -> DataArray { let mut a = DataArray::new(); for s in v { a.push_string(s); } a }
// the op in progress: in1/in3 and the collected list outputs are the stepper's own containers
fn copy_inprog(ip: &DataObject) -> DataObject {
  let mut n = shallow_obj(ip);
  for k in ["in1", "in3"] { if is_obj(ip, k) { n.put_object(k, shallow_obj(&ip.get_object(k))); } }
  if is_obj(ip, "out3") {
    let lo = if is_arr(ip, "list_out") { strs(&ip.get_array("list_out")) } else { Vec::new() };
    let mut o = shallow_obj(&ip.get_object("out3"));
    for k in &lo { if is_arr(&o, k) { let a = shallow_arr(&o.get_array(k)); o.put_array(k, a); } }
    n.put_object("out3", o);
  }
  n
}
pub fn copy_frame(f: &DataObject) -> DataObject {
  let mut n = shallow_obj(f);
  for k in ["wires", "syncs", "done", "prio", "results"] { if is_arr(f, k) { n.put_array(k, shallow_arr(&f.get_array(k))); } }
  for k in ["cur", "pending", "out"] { if is_obj(f, k) { n.put_object(k, shallow_obj(&f.get_object(k))); } }
  for k in ["inputs", "dlv"] {
    if is_arr(f, k) {
      let mut na = DataArray::new();
      for v in f.get_array(k).objects() { na.push_object(shallow_obj(&v.object())); }
      n.put_array(k, na);
    }
  }
  if is_obj(f, "inprog") { n.put_object("inprog", copy_inprog(&f.get_object("inprog"))); }
  n
}
pub fn case_at(body: &DataObject, idx: i64) -> Option<DataObject> {
  let mut c = body.clone();
  let mut i = 0;
  while i < idx { if c.has("nextcase") { c = c.get_object("nextcase"); i += 1; } else { return None; } }
  Some(c)
}
fn n_syncs(case: &DataObject) -> usize { if case.has("syncs") { case.get_array("syncs").len() } else { 0 } }
fn sync_valid(s: i64, d: i64, n: usize) -> bool { s >= 0 && d >= 0 && s != d && (s as usize) < n && (d as usize) < n }
fn bools(n: usize) -> DataArray { let mut a = DataArray::new(); for _ in 0..n { a.push_boolean(false); } a }
fn dest_names(case: &DataObject) -> Vec<String> {
  let mut v = Vec::new();
  for c in case.get_array("cons").objects() { v.push(c.object().get_array("dest").get_string(1)); }
  v
}
fn syncs_ok(case: &DataObject, f: &DataObject, i: usize) -> bool {
  if !case.has("syncs") { return true; }
  let n = case.get_array("cmds").len();
  let done = f.get_array("done");
  for s in case.get_array("syncs").objects() {
    let s = s.object();
    let (a, b) = (s.get_int("src"), s.get_int("dest"));
    if b == i as i64 && sync_valid(a, b, n) && !done.get_boolean(a as usize) { return false; }
  }
  true
}
fn op_ready(case: &DataObject, f: &DataObject, i: usize) -> bool {
  if f.get_array("done").get_boolean(i) { return false; }
  let dv = f.get_array("dlv").get_object(i);
  let op = case.get_array("cmds").get_object(i);
  for name in op.get_object("in").keys() { if !dv.has(&name) { return false; } }
  syncs_ok(case, f, i)
}
fn wire_ready(case: &DataObject, f: &DataObject, k: usize) -> bool {
  if f.get_array("wires").get_boolean(k) { return false; }
  let si = case.get_array("cons").get_object(k).get_array("src").get_int(0);
  if si == -1 { return true; }
  let n = case.get_array("cmds").len();
  if si < 0 || si as usize >= n { return true; }
  f.get_array("done").get_boolean(si as usize)
}
fn sync_ready(case: &DataObject, f: &DataObject, k: usize) -> bool {
  if f.get_array("syncs").get_boolean(k) { return false; }
  let s = case.get_array("syncs").get_object(k);
  let (a, b) = (s.get_int("src"), s.get_int("dest"));
  let n = case.get_array("cmds").len();
  if !sync_valid(a, b, n) { return true; }
  f.get_array("done").get_boolean(a as usize)
}
pub fn item(kind: &str, i: usize) -> DataObject { let mut o = DataObject::new(); o.put_string("kind", kind); o.put_int("index", i as i64); o }
pub fn ready_set(case: &DataObject, f: &DataObject) -> DataArray {
  let mut a = DataArray::new();
  for k in 0..case.get_array("cons").len() { if wire_ready(case, f, k) { a.push_object(item("wire", k)); } }
  for k in 0..n_syncs(case) { if sync_ready(case, f, k) { a.push_object(item("sync", k)); } }
  for i in 0..case.get_array("cmds").len() { if op_ready(case, f, i) { a.push_object(item("op", i)); } }
  a
}
// the ready set of the TOP frame (only the top frame steps)
pub fn ready_now(f: &DataObject) -> DataArray {
  let phase = f.get_string("phase");
  let mut a = DataArray::new();
  if phase == "ended" { return a; }
  if phase != "run" { let mut o = DataObject::new(); o.put_string("kind", "auto"); o.put_string("phase", &phase); a.push_object(o); return a; }
  if is_obj(f, "inprog") {
    let ip = f.get_object("inprog");
    let mut o = DataObject::new(); o.put_string("kind", "auto"); o.put_string("phase", "op");
    o.put_int("op", ip.get_int("op")); o.put_string("stage", &ip.get_string("stage"));
    a.push_object(o); return a;
  }
  if is_obj(f, "pending") {
    let p = f.get_object("pending");
    let mut o = DataObject::new(); o.put_string("kind", "effect"); o.put_string("rule", &p.get_string("rule")); o.put_int("op", p.get_int("op"));
    a.push_object(o); return a;
  }
  match case_at(&f.get_object("body"), f.get_int("case")) { Some(c) => ready_set(&c, f), None => a }
}
fn prio_front(f: &mut DataObject, i: usize) {
  let old = f.get_array("prio");
  let mut a = DataArray::new(); a.push_int(i as i64);
  for d in old.objects() { if d.int() != i as i64 { a.push_property(d); } }
  f.put_array("prio", a);
}
fn prio_remove(f: &mut DataObject, i: usize) {
  let old = f.get_array("prio");
  let mut a = DataArray::new();
  for d in old.objects() { if d.int() != i as i64 { a.push_property(d); } }
  f.put_array("prio", a);
}
fn diag_case(case: &DataObject) -> DataArray {
  let mut a = DataArray::new();
  let cmds = case.get_array("cmds");
  let mut exact: Vec<(i64, String)> = Vec::new();
  let mut names: Vec<String> = Vec::new();
  for c in case.get_array("cons").objects() { let d = c.object().get_array("dest"); exact.push((d.get_int(0), d.get_string(1))); names.push(d.get_string(1)); }
  for i in 0..cmds.len() {
    for name in cmds.get_object(i).get_object("in").keys() {
      let fan = exact.iter().filter(|(x, y)| *x == i as i64 && *y == name).count();
      if fan > 1 { a.push_string(&format!("I-1 op {} input '{}' has {} incoming wires: it runs once (flowlang would re-run it)", i, name, fan)); }
      if fan == 0 && names.contains(&name) { a.push_string(&format!("I-1 op {} input '{}' is awaited by name but no wire targets it: the op can never be ready", i, name)); }
    }
  }
  a
}
pub fn ev(kind: &str, f: &DataObject) -> DataObject {
  let mut e = DataObject::new();
  e.put_string("type", kind);
  e.put_array("frame", if is_arr(f, "path") { f.get_array("path").deep_copy() } else { DataArray::new() });
  e.put_int("case", f.get_int("case"));
  e
}
fn err_ev(f: &mut DataObject, code: &str, msg: String, op: Option<usize>) -> DataObject {
  let mut e = ev("error", f);
  e.put_string("code", code); e.put_string("msg", &msg);
  if let Some(i) = op { e.put_int("op", i as i64); }
  f.put_string("phase", "ended");
  e
}
pub fn new_frame(args: DataObject, kind: &str, body: DataObject, path: DataArray) -> DataObject {
  let mut f = DataObject::new();
  f.put_string("kind", kind);
  f.put_object("body", body);
  f.put_array("path", path);
  f.put_object("args", args);
  f.put_object("out", DataObject::new());
  f.put_int("case", 0);
  f.put_boolean("finish", false);
  f.put_string("phase", "enter");
  f.put_null("pending");
  f.put_null("inprog");
  f.put_string("outcome", "");
  f
}
// seed: fresh case state; inputs whose NAME is no wire's dest name anywhere in the case are pre-delivered (null).
fn seed_case(case: &DataObject, f: &mut DataObject) -> DataArray {
  let cmds = case.get_array("cmds");
  let n = cmds.len();
  f.put_array("wires", bools(case.get_array("cons").len()));
  f.put_array("syncs", bools(n_syncs(case)));
  f.put_array("done", bools(n));
  let dn = dest_names(case);
  let mut results = DataArray::new(); let mut inputs = DataArray::new(); let mut dlv = DataArray::new();
  let mut pre = DataArray::new();
  for i in 0..n {
    results.push_property(Data::DNull);
    let op = cmds.get_object(i);
    let mut iv = DataObject::new(); let mut dv = DataObject::new();
    for name in op.get_object("in").keys() {
      if !dn.contains(&name) {
        iv.put_null(&name); dv.put_boolean(&name, true);
        let mut p = DataArray::new(); p.push_int(i as i64); p.push_string(&name); pre.push_array(p);
      }
    }
    inputs.push_object(iv); dlv.push_object(dv);
  }
  f.put_array("results", results); f.put_array("inputs", inputs); f.put_array("dlv", dlv);
  f.put_null("pending");
  f.put_null("inprog");
  let mut cur = DataObject::new(); cur.put_string("pass", "wire"); cur.put_int("k", 0); f.put_object("cur", cur);
  f.put_array("prio", DataArray::new());
  // case-start rule: no awaited input AND no incoming synchro; flowlang runs these first, in index order
  let mut prio = DataArray::new();
  for i in 0..n { if op_ready(case, f, i) { prio.push_int(i as i64); } }
  f.put_array("prio", prio);
  pre
}
// DEFAULT pick = flowlang's sweep: case-start ops, then wire passes and synchro passes from the cursor;
// an op made ready by a delivery/synchro is picked next (flowlang runs it inside the pass).
pub fn default_pick(case: &DataObject, f: &mut DataObject) -> Option<(String, usize)> {
  let prio = f.get_array("prio");
  let mut keep = DataArray::new();
  let mut chosen: Option<usize> = None;
  for d in prio.objects() {
    let i = d.int() as usize;
    if op_ready(case, f, i) { if chosen.is_none() { chosen = Some(i); } else { keep.push_int(i as i64); } }
  }
  f.put_array("prio", keep);
  if let Some(i) = chosen { return Some(("op".to_string(), i)); }
  let nw = case.get_array("cons").len();
  let ns = n_syncs(case);
  let mut cur = f.get_object("cur");
  let mut pass = cur.get_string("pass");
  let mut k = cur.get_int("k") as usize;
  for _round in 0..3 {
    if pass == "wire" {
      let mut j = k;
      while j < nw { if wire_ready(case, f, j) { cur.put_string("pass", "wire"); cur.put_int("k", (j + 1) as i64); return Some(("wire".to_string(), j)); } j += 1; }
      pass = "sync".to_string(); k = 0;
    } else {
      let mut j = k;
      while j < ns { if sync_ready(case, f, j) { cur.put_string("pass", "sync"); cur.put_int("k", (j + 1) as i64); return Some(("sync".to_string(), j)); } j += 1; }
      pass = "wire".to_string(); k = 0;
    }
  }
  cur.put_string("pass", &pass); cur.put_int("k", k as i64);
  let n = case.get_array("cmds").len();
  for i in 0..n { if op_ready(case, f, i) { return Some(("op".to_string(), i)); } }
  None
}
fn unfired(f: &DataObject) -> (DataArray, DataArray, DataArray) {
  let wires = f.get_array("wires"); let syncs = f.get_array("syncs"); let done = f.get_array("done");
  let mut uw = DataArray::new(); for k in 0..wires.len() { if !wires.get_boolean(k) { uw.push_int(k as i64); } }
  let mut us = DataArray::new(); for k in 0..syncs.len() { if !syncs.get_boolean(k) { us.push_int(k as i64); } }
  let mut nr = DataArray::new(); for i in 0..done.len() { if !done.get_boolean(i) { nr.push_int(i as i64); } }
  (uw, us, nr)
}
fn stall_ev(f: &mut DataObject, uw: DataArray, us: DataArray, nr: DataArray) -> DataObject {
  f.put_string("phase", "ended");
  let mut e = ev("stall", f);
  e.put_array("wires", uw); e.put_array("syncs", us); e.put_array("ops", nr);
  e
}
// the root frame's case ran dry: done (everything fired) or stall
fn end_of_case(f: &mut DataObject) -> DataObject {
  let (uw, us, nr) = unfired(f);
  if uw.len() > 0 || us.len() > 0 { return stall_ev(f, uw, us, nr); }
  f.put_string("phase", "ended");
  let mut e = ev("done", f);
  e.put_string("outcome", "ok"); put_copy(&mut e, "out", f.get_property("out")); e.put_boolean("fail", false);
  e.put_array("never_ready", nr);
  f.put_string("outcome", "ok");
  e
}
// a nested frame's case ran dry: leave with outcome ok, or stall (which ends the whole run)
fn end_nested(frames: &mut DataArray) -> DataObject {
  let mut f = frames.get_object(frames.len() - 1);
  let (uw, us, nr) = unfired(&f);
  if uw.len() > 0 || us.len() > 0 { return stall_ev(&mut f, uw, us, nr); }
  leave(frames, "ok", nr)
}
fn deliver(case: &DataObject, f: &mut DataObject, k: usize) -> DataObject {
  let cmds = case.get_array("cmds"); let n = cmds.len();
  let con = case.get_array("cons").get_object(k);
  let (src, dst) = (con.get_array("src"), con.get_array("dest"));
  let (si, sn, di, dn) = (src.get_int(0), src.get_string(1), dst.get_int(0), dst.get_string(1));
  let val: Data;
  if si == -1 { let a = f.get_object("args"); val = if a.has(&sn) { a.get_property(&sn) } else { Data::DNull }; }
  else if si < 0 || si as usize >= n { return err_ev(f, "panic", format!("wire {} has src index {} out of range", k, si), None); }
  else {
    let r = f.get_array("results").get_property(si as usize);
    if !r.is_object() || !r.object().has(&sn) {
      return err_ev(f, "absent-output", format!("wire {} reads output '{}' of op {}, which its result lacks (flowlang panics in get_property)", k, sn, si), Some(si as usize));
    }
    val = r.object().get_property(&sn);
  }
  if di != -2 && (di < 0 || di as usize >= n) { return err_ev(f, "panic", format!("wire {} has dest index {} out of range", k, di), None); }
  f.get_array("wires").put_boolean(k, true);
  let mut e = ev("deliver", f);
  e.put_int("wire", k as i64);
  put_copy(&mut e, "value", val.clone());
  let mut d = DataArray::new(); d.push_int(di); d.push_string(&dn); e.put_array("dest", d);
  if di == -2 { f.get_object("out").set_property(&dn, val); return e; }
  let du = di as usize;
  let op = cmds.get_object(du);
  if op.get_string("type") == "undefined" {
    f.get_array("done").put_boolean(du, true);
    f.get_array("results").set_property(du, Data::DNull);
    e.put_boolean("marked_done", true);
    return e;
  }
  if !op.get_object("in").has(&dn) { return err_ev(f, "panic", format!("wire {} targets input '{}', which op {} does not have", k, dn, du), Some(du)); }
  if f.get_array("done").get_boolean(du) {
    e.put_boolean("ignored", true);
    e.put_string("diag", &format!("I-1: op {} already ran; this second delivery to '{}' does not re-run it (flowlang would run it again)", du, dn));
    return e;
  }
  let mut dv = f.get_array("dlv").get_object(du);
  if dv.has(&dn) { e.put_string("diag", &format!("I-1: input '{}' of op {} was delivered twice before the op ran; the last value wins", dn, du)); }
  f.get_array("inputs").get_object(du).set_property(&dn, val);
  dv.put_boolean(&dn, true);
  if op_ready(case, f, du) { prio_front(f, du); e.put_int("ready_op", du as i64); }
  e
}
fn fire_sync(case: &DataObject, f: &mut DataObject, k: usize) -> DataObject {
  let n = case.get_array("cmds").len();
  let s = case.get_array("syncs").get_object(k);
  let (a, b) = (s.get_int("src"), s.get_int("dest"));
  let valid = sync_valid(a, b, n);
  f.get_array("syncs").put_boolean(k, true);
  let mut e = ev("sync", f);
  e.put_int("sync", k as i64); e.put_int("src", a); e.put_int("dest", b); e.put_boolean("inert", !valid);
  if valid && op_ready(case, f, b as usize) { prio_front(f, b as usize); e.put_int("ready_op", b); }
  e
}
fn match_flag(ctype: &str, v: &str, d: Data) -> Result<bool, Fault> {
  let bad = |t: &str| fault("panic", format!("match literal '{}' does not parse as {}", v, t));
  Ok(match ctype {
    "int" => { if !d.is_int() { false } else { match v.parse::<i64>() { Ok(x) => d.int() == x, Err(_) => return Err(bad("int")) } } }
    "decimal" => { if !d.is_float() { false } else { match v.parse::<f64>() { Ok(x) => d.float() == x, Err(_) => return Err(bad("decimal")) } } }
    "boolean" => { if !d.is_boolean() { false } else { match v.parse::<bool>() { Ok(x) => d.boolean() == x, Err(_) => return Err(bad("boolean")) } } }
    "string" => { if !d.is_string() { false } else { d.string() == v } }
    "null" => d.clone().is_null(),
    _ => false,
  })
}
// list inputs, list outputs and loop outputs of an op (flowlang evaluate)
fn modes(op: &DataObject) -> Result<(Vec<String>, Vec<String>, Vec<(String, String)>), Fault> {
  let mut list_in: Vec<String> = Vec::new();
  for (k, d) in op.get_object("in").objects() { if d.is_object() && d.object().has("mode") && d.object().get_string("mode") == "list" { list_in.push(k); } }
  let mut list_out: Vec<String> = Vec::new();
  let mut loop_out: Vec<(String, String)> = Vec::new();
  for (k, d) in op.get_object("out").objects() {
    if !d.is_object() { continue; }
    let nd = d.object();
    let mode = if nd.has("mode") { nd.get_string("mode") } else { "".to_string() };
    if mode == "list" { list_out.push(k); }
    else if mode == "loop" {
      if !(nd.has("loop") && nd.get_property("loop").is_string()) { return Err(fault("panic", format!("loop output '{}' names no input (I-8)", k))); }
      loop_out.push((k, nd.get_string("loop")));
    }
  }
  Ok((list_in, list_out, loop_out))
}
fn list_count(in1: &DataObject, list_in: &Vec<String>) -> Result<usize, Fault> {
  let mut count = 0usize;
  for (j, k) in list_in.iter().enumerate() {
    let d = if in1.has(k) { in1.get_property(k) } else { Data::DNull };
    if !d.is_array() { return Err(fault("panic", format!("list input '{}' is not an array", k))); }
    let l = d.array().len();
    if j == 0 || l < count { count = l; }
  }
  Ok(count)
}
fn build_in3(in1: &DataObject, list_in: &Vec<String>, i: usize) -> Result<DataObject, Fault> {
  let mut in3 = DataObject::new();
  for (k, d) in in1.objects() {
    if !list_in.contains(&k) { in3.set_property(&k, d); }
    else {
      let a = d.array();
      if i >= a.len() { return Err(fault("panic", format!("iteration {} indexes past the end of list input '{}' (a loop output with an empty list input panics flowlang)", i, k))); }
      in3.set_property(&k, a.get_property(i));
    }
  }
  Ok(in3)
}
// Does this op open a frame? A local with localdata, or a command whose callee is a FLOW (any library).
fn frame_spec(op: &DataObject) -> Result<Option<DataObject>, Fault> {
  let typ = op.get_string("type");
  if typ == "local" {
    if !is_obj(op, "localdata") { return Ok(None); }
    let mut s = DataObject::new(); s.put_string("kind", "local"); s.put_object("body", op.get_object("localdata")); s.put_array("keys", DataArray::new());
    return Ok(Some(s));
  }
  if typ != "command" { return Ok(None); }
  if !op.has("cmd") { return Err(fault("panic", "command op has no cmd".to_string())); }
  let cmdstr = op.get_string("cmd");
  let sa: Vec<String> = cmdstr.split(':').map(|s| s.to_string()).collect();
  if sa.len() < 3 { return Err(fault("panic", format!("command op cmd '{}' is not lib:ctl:cmd", cmdstr))); }
  let lib = sa[0].clone(); let id = sa[2].clone();
  let (l2, i2) = (lib.clone(), id.clone());
  let sub = match panic::catch_unwind(panic::AssertUnwindSafe(move || Command::new(&l2, &i2))) {
    Ok(c) => c,
    Err(p) => return Err(fault("panic", format!("command '{}' could not be loaded: {}", cmdstr, panic_text(&p)))),
  };
  if sub.lang != "flow" { return Ok(None); }
  // the callee's SAVED flow; raw args (no cast_params) and no check_auth - the parent call is the gate
  let store = DataStore::new();
  let data = store.get_data(&lib, &id).get_object("data");
  let codename = data.get_string("flow");
  let fl = store.get_data(&lib, &codename).get_object("data").get_object("flow");
  let keys: Vec<String> = fl.get_object("output").keys();
  let mut s = DataObject::new();
  s.put_string("kind", "command"); s.put_object("body", fl); s.put_array("keys", vec_arr(&keys));
  s.put_string("lib", &lib); s.put_string("cmd", &cmdstr);
  Ok(Some(s))
}
// One evaluation of an op body that opens NO frame (flowlang evaluate_operation, minus the condition).
fn op_body(op: &DataObject, in1: DataObject) -> Result<(DataObject, bool), Fault> {
  let typ = op.get_string("type");
  let v = op.get_string("name");
  let outs: Vec<String> = op.get_object("out").keys();
  let mut out = DataObject::new();
  let mut b = true;
  if typ == "primitive" {
    let a = in1.clone(); let vv = v.clone();
    match panic::catch_unwind(panic::AssertUnwindSafe(move || Primitive::new(&vv).execute(a))) {
      Ok(o) => { out = o; }
      Err(p) => { return Err(fault("panic", format!("primitive '{}' panicked: {}", v, panic_text(&p)))); }
    }
  } else if typ == "local" {
    // a local with no localdata opens no frame: every output null, b = true
    for k in &outs { out.put_null(k); }
  } else if typ == "constant" {
    if !op.has("ctype") { return Err(fault("panic", "constant op has no ctype".to_string())); }
    let ctype = op.get_string("ctype");
    for k in &outs {
      let bad = |e: String| fault("panic", format!("constant '{}' does not parse as {}: {}", v, ctype, e));
      match ctype.as_str() {
        "int" => match v.parse::<i64>() { Ok(x) => out.put_int(k, x), Err(e) => return Err(bad(e.to_string())) },
        "decimal" => match v.parse::<f64>() { Ok(x) => out.put_float(k, x), Err(e) => return Err(bad(e.to_string())) },
        "boolean" => match v.parse::<bool>() { Ok(x) => out.put_boolean(k, x), Err(e) => return Err(bad(e.to_string())) },
        "string" => match DataObject::try_from_string(&format!("{{\"v\":\"{}\"}}", v)) { Ok(o) => out.put_string(k, &o.get_string("v")), Err(_) => return Err(bad("bad string literal".to_string())) },
        "object" => { let vv = v.clone(); match panic::catch_unwind(panic::AssertUnwindSafe(move || DataObject::from_string(&vv))) { Ok(o) => out.put_object(k, o), Err(p) => return Err(bad(panic_text(&p))) } },
        "array" => { let vv = v.clone(); match panic::catch_unwind(panic::AssertUnwindSafe(move || DataArray::from_string(&vv))) { Ok(o) => out.put_array(k, o), Err(p) => return Err(bad(panic_text(&p))) } },
        _ => out.put_null(k),
      }
    }
  } else if typ == "command" {
    // a rust/python callee: one opaque eval, its single key `a` maps onto the op's output
    if !op.has("cmd") { return Err(fault("panic", "command op has no cmd".to_string())); }
    let cmdstr = op.get_string("cmd");
    let sa: Vec<String> = cmdstr.split(':').map(|s| s.to_string()).collect();
    if sa.len() < 3 { return Err(fault("panic", format!("command op cmd '{}' is not lib:ctl:cmd", cmdstr))); }
    let (l2, i2) = (sa[0].clone(), sa[2].clone());
    let sub = match panic::catch_unwind(panic::AssertUnwindSafe(move || Command::new(&l2, &i2))) {
      Ok(c) => c,
      Err(p) => return Err(fault("panic", format!("command '{}' could not be loaded: {}", cmdstr, panic_text(&p)))),
    };
    let mut params = DataObject::new();
    for (k, d) in in1.objects() { params.set_property(&k, d); }
    let keys: Vec<String> = vec!["a".to_string()];
    let s2 = &sub; let p2 = params.clone();
    let res = match panic::catch_unwind(panic::AssertUnwindSafe(move || s2.execute(p2))) {
      Ok(Ok(o)) => Some(o),
      Ok(Err(_)) => None,
      Err(p) => return Err(fault("panic", format!("command '{}' panicked: {}", cmdstr, panic_text(&p)))),
    };
    match res {
      None => { b = false; }
      Some(r) => {
        for (i, key1) in outs.iter().enumerate() {
          if i >= keys.len() { return Err(fault("absent-output", format!("command op output '{}' has no positional counterpart among the callee's {} output(s) (I-7)", key1, keys.len()))); }
          let key2 = &keys[i];
          if !r.has(key2) { return Err(fault("absent-output", format!("the callee's result lacks '{}' (flowlang panics in get_property)", key2))); }
          out.set_property(key1, r.get_property(key2));
        }
      }
    }
  } else if typ == "persistent" {
    let mut g = DataStore::globals();
    let ks = in1.clone().keys();
    if ks.len() > 0 { g.set_property(&v, in1.get_property(&ks[0])); }
    let value = if g.has(&v) { g.get_property(&v) } else { Data::DNull };
    for k in &outs { out.set_property(k, value.clone()); }
  } else if typ == "match" {
    let ks = in1.clone().keys();
    if ks.is_empty() { return Err(fault("panic", "a match op needs one input (I-5)".to_string())); }
    if !op.has("ctype") { return Err(fault("panic", "match op has no ctype".to_string())); }
    b = match_flag(&op.get_string("ctype"), &v, in1.get_property(&ks[0]))?;
  }
  // undefined and unknown types: flowlang prints UNIMPLEMENTED and the op yields {}
  Ok((out, b))
}
// A closed frame's result as the op's result: fail = b false + EMPTY result; a command maps by position (I-7).
fn closing_result(op: &DataObject, ip: &DataObject) -> Result<(DataObject, bool, bool), Fault> {
  let r = ip.get_object("res");
  if r.get_string("oc") == "fail" { return Ok((DataObject::new(), false, false)); }
  let o = r.get_object("out");
  if op.get_string("type") == "local" { return Ok((o, true, r.get_boolean("fin"))); }
  let keys = strs(&ip.get_array("keys"));
  let outs: Vec<String> = op.get_object("out").keys();
  let mut out = DataObject::new();
  for (i, key1) in outs.iter().enumerate() {
    if i >= keys.len() { return Err(fault("absent-output", format!("command op output '{}' has no positional counterpart among the callee's {} output(s) (I-7)", key1, keys.len()))); }
    let key2 = &keys[i];
    if !o.has(key2) { return Err(fault("absent-output", format!("the callee's result lacks '{}' (flowlang panics in get_property)", key2))); }
    out.set_property(key1, o.get_property(key2));
  }
  Ok((out, true, false))
}
// The op's condition; a finish rule on it marks the ENCLOSING frame's flag (via ffin and the finish effect).
fn condition(op: &DataObject, b: bool, ffin: &mut bool) -> Option<String> {
  if op.get_string("type") == "constant" || !is_obj(op, "condition") { return None; }
  let c = op.get_object("condition");
  if c.get_boolean("value") != b { return None; }
  let r = c.get_string("rule");
  if r == "next" || r == "terminate" || r == "fail" { return Some(r); }
  if r == "finish" { *ffin = true; return Some(r); }
  None
}
fn op_ev(f: &DataObject, op: &DataObject, i: usize, in1: &DataObject) -> DataObject {
  let mut e = ev("eval", f);
  e.put_int("op", i as i64); e.put_string("optype", &op.get_string("type")); e.put_object("in", in1.deep_copy());
  e
}
// The op's final eval: done unless next/terminate/fail fired; a fired rule's effect is the next step.
fn finish_op(f: &mut DataObject, mut e: DataObject, i: usize, out: DataObject, b: bool, rule: Option<String>, ffin: bool, iters: Option<i64>) -> DataObject {
  let done = !matches!(rule.as_deref(), Some("next") | Some("terminate") | Some("fail"));
  e.put_object("out", out.deep_copy());
  e.put_boolean("b", b);
  match &rule { Some(r) => e.put_string("rule", r), None => e.put_null("rule") }
  e.put_boolean("done", done);
  match iters { Some(n) => e.put_int("iters", n), None => e.put_null("iters") }
  if done { f.get_array("done").put_boolean(i, true); f.get_array("results").put_object(i, out); }
  if let Some(r) = &rule { let mut p = DataObject::new(); p.put_string("rule", r); p.put_int("op", i as i64); f.put_object("pending", p); }
  if ffin && rule.as_deref() != Some("finish") { f.put_boolean("finish", true); e.put_boolean("finish_set", true); }
  f.put_null("inprog");
  e
}
fn push_frame(frames: &mut DataArray, spec: &DataObject, args: DataObject, op: usize, iter: Data) -> DataObject {
  let n = frames.len();
  let mut parent = frames.get_object(n - 1);
  if n >= 64 { return err_ev(&mut parent, "depth", "frames nested more than 64 deep (recursive flow?)".to_string(), Some(op)); }
  let mut path = shallow_arr(&parent.get_array("path"));
  let mut at = DataArray::new(); at.push_int(op as i64); at.push_property(iter.clone()); path.push_array(at);
  let kind = spec.get_string("kind");
  let mut nf = new_frame(args.clone(), &kind, spec.get_object("body"), path);
  nf.put_string("phase", "seed");
  let mut e = ev("enter", &nf);
  e.put_string("kind", &kind); e.put_int("op", op as i64); e.set_property("iter", iter);
  e.put_object("args", args.deep_copy());
  if kind == "command" {
    let (l, c) = (spec.get_string("lib"), spec.get_string("cmd"));
    nf.put_string("lib", &l); nf.put_string("cmd", &c);
    e.put_string("lib", &l); e.put_string("cmd", &c);
  }
  frames.push_object(nf);
  e
}
// pop the top frame; its outcome and out become the parent's op-in-progress result (stage closing)
fn leave(frames: &mut DataArray, oc: &str, nr: DataArray) -> DataObject {
  let n = frames.len();
  let mut child = frames.get_object(n - 1);
  child.put_string("phase", "ended"); child.put_string("outcome", oc);
  let mut e = ev("leave", &child);
  e.put_string("kind", &child.get_string("kind"));
  e.put_string("outcome", oc);
  put_copy(&mut e, "out", child.get_property("out"));
  e.put_boolean("fail", oc == "fail");
  e.put_boolean("finish", child.get_boolean("finish"));
  let path = child.get_array("path");
  if path.len() > 0 { let at = path.get_array(path.len() - 1); e.put_int("op", at.get_int(0)); e.set_property("iter", at.get_property(1)); }
  if child.has("lib") { e.put_string("lib", &child.get_string("lib")); e.put_string("cmd", &child.get_string("cmd")); }
  if oc == "ok" { e.put_array("never_ready", nr); }
  frames.remove_property(n - 1);
  let parent = frames.get_object(n - 2);
  let mut ip = parent.get_object("inprog");
  let mut r = DataObject::new();
  r.put_string("oc", oc); r.put_object("out", child.get_object("out")); r.put_boolean("fin", child.get_boolean("finish"));
  ip.put_object("res", r); ip.put_string("stage", "closing");
  e
}
fn start_op(frames: &mut DataArray, case: &DataObject, i: usize) -> DataObject {
  let mut f = frames.get_object(frames.len() - 1);
  prio_remove(&mut f, i);
  let op = case.get_array("cmds").get_object(i);
  let in1 = shallow_obj(&f.get_array("inputs").get_object(i));
  let (list_in, list_out, loop_out) = match modes(&op) { Ok(m) => m, Err((c, m)) => return err_ev(&mut f, &c, format!("op {}: {}", i, m), Some(i)) };
  if list_in.is_empty() && loop_out.is_empty() {
    return match frame_spec(&op) {
      Err((c, m)) => err_ev(&mut f, &c, format!("op {}: {}", i, m), Some(i)),
      Ok(Some(spec)) => {
        let mut ip = DataObject::new();
        ip.put_int("op", i as i64); ip.put_boolean("iterated", false); ip.put_string("stage", "frame");
        ip.put_boolean("ffin", false); ip.put_array("keys", spec.get_array("keys"));
        f.put_object("inprog", ip);
        push_frame(frames, &spec, in1, i, Data::DNull)
      }
      Ok(None) => {
        let e = op_ev(&f, &op, i, &in1);
        match op_body(&op, in1) {
          Err((c, m)) => err_ev(&mut f, &c, format!("op {}: {}", i, m), Some(i)),
          Ok((out, b)) => { let mut ffin = false; let rule = condition(&op, b, &mut ffin); finish_op(&mut f, e, i, out, b, rule, ffin, None) }
        }
      }
    };
  }
  let count = match list_count(&in1, &list_in) { Ok(c) => c, Err((c, m)) => return err_ev(&mut f, &c, format!("op {}: {}", i, m), Some(i)) };
  let outs: Vec<String> = op.get_object("out").keys();
  let mut out3 = DataObject::new();
  for k in &list_out { out3.put_array(k, DataArray::new()); }
  if loop_out.is_empty() && count == 0 {
    // zero iterations: no body, no condition, list outputs [], the others null
    let e = op_ev(&f, &op, i, &in1);
    for k in &outs { if !out3.has(k) { out3.put_null(k); } }
    return finish_op(&mut f, e, i, out3, true, None, false, Some(0));
  }
  let mut lp = DataArray::new();
  for (a, b) in &loop_out { let mut p = DataArray::new(); p.push_string(a); p.push_string(b); lp.push_array(p); }
  let mut ip = DataObject::new();
  ip.put_int("op", i as i64); ip.put_boolean("iterated", true); ip.put_string("stage", "iter");
  ip.put_int("i", 0); ip.put_int("iters", 0); ip.put_int("nl", list_in.len() as i64); ip.put_int("count", count as i64);
  ip.put_object("in1", in1); ip.put_object("out3", out3);
  ip.put_boolean("lastb", true); ip.put_null("last_rule"); ip.put_boolean("ofin", false); ip.put_boolean("ffin", false);
  ip.put_array("list_in", vec_arr(&list_in)); ip.put_array("list_out", vec_arr(&list_out)); ip.put_array("loop_out", lp);
  ip.put_array("keys", DataArray::new());
  f.put_object("inprog", ip);
  iter_step(&mut f, &op)
}
// an iteration begins: its inputs (each list input contributes its element)
fn iter_step(f: &mut DataObject, op: &DataObject) -> DataObject {
  let mut ip = f.get_object("inprog");
  let i = ip.get_int("op") as usize;
  let li = strs(&ip.get_array("list_in"));
  let in3 = match build_in3(&ip.get_object("in1"), &li, ip.get_int("i") as usize) { Ok(x) => x, Err((c, m)) => return err_ev(f, &c, format!("op {}: {}", i, m), Some(i)) };
  let mut e = ev("iter", f);
  e.put_int("op", i as i64); e.put_string("optype", &op.get_string("type"));
  e.put_int("i", ip.get_int("iters"));
  if ip.get_int("nl") > 0 { e.put_int("n", ip.get_int("count")); } else { e.put_null("n"); }
  e.put_object("in", in3.deep_copy());
  ip.put_object("in3", in3); ip.put_string("stage", "body");
  e
}
// one iteration's result: condition per iteration, collect outputs, loop feedback, last-iteration test
fn after_iter(f: &mut DataObject, op: &DataObject, i: usize, res: DataObject, b: bool, fin: bool) -> DataObject {
  let mut ip = f.get_object("inprog");
  let it = ip.get_int("iters");
  ip.put_int("iters", it + 1);
  if fin { ip.put_boolean("ofin", true); }
  let mut ffin = ip.get_boolean("ffin");
  let rule = condition(op, b, &mut ffin);
  ip.put_boolean("ffin", ffin);
  ip.put_boolean("lastb", b);
  let mut e = ev("eval", f);
  e.put_int("op", i as i64); e.put_string("optype", &op.get_string("type")); e.put_int("iter", it);
  e.put_object("in", ip.get_object("in3").deep_copy());
  if matches!(rule.as_deref(), Some("next") | Some("terminate") | Some("fail")) { return finish_op(f, e, i, res, b, rule, ffin, Some(it + 1)); }
  if rule.is_some() { ip.put_string("last_rule", "finish"); e.put_boolean("finish_fired", true); }
  let lo = strs(&ip.get_array("list_out"));
  let mut lp: Vec<(String, String)> = Vec::new();
  for p in ip.get_array("loop_out").objects() { let p = p.array(); lp.push((p.get_string(0), p.get_string(1))); }
  let mut out3 = ip.get_object("out3");
  let mut in1 = ip.get_object("in1");
  for k in op.get_object("out").keys() {
    if !res.has(&k) { return err_ev(f, "absent-output", format!("op {}: iteration {} result lacks output '{}' (flowlang panics in get_property)", i, it, k), Some(i)); }
    let dp = res.get_property(&k);
    if lo.contains(&k) { out3.get_array(&k).push_property(dp); }
    else {
      out3.set_property(&k, dp.clone());
      if let Some((_, lf)) = lp.iter().find(|(a, _)| *a == k) { in1.set_property(lf, dp); }
    }
  }
  let nl = ip.get_int("nl"); let count = ip.get_int("count");
  let mut last = ip.get_boolean("ofin");
  if !last && nl > 0 { let ni = ip.get_int("i") + 1; ip.put_int("i", ni); if ni == count { last = true; } }
  if last {
    e.put_object("iter_out", res.deep_copy());
    let lr = if ip.has("last_rule") && ip.get_property("last_rule").is_string() { Some(ip.get_string("last_rule")) } else { None };
    let lb = ip.get_boolean("lastb");
    return finish_op(f, e, i, out3, lb, lr, ffin, Some(it + 1));
  }
  e.put_object("out", res.deep_copy()); e.put_boolean("b", b); e.put_null("rule"); e.put_boolean("done", false); e.put_null("iters");
  ip.put_string("stage", "iter");
  e
}
// the top frame has an op in progress: its next sub-step
fn continue_op(frames: &mut DataArray, case: &DataObject) -> DataObject {
  let mut f = frames.get_object(frames.len() - 1);
  let mut ip = f.get_object("inprog");
  let i = ip.get_int("op") as usize;
  let op = case.get_array("cmds").get_object(i);
  let stage = ip.get_string("stage");
  if stage == "iter" { return iter_step(&mut f, &op); }
  if stage == "body" {
    let in3 = ip.get_object("in3");
    return match frame_spec(&op) {
      Err((c, m)) => err_ev(&mut f, &c, format!("op {}: {}", i, m), Some(i)),
      Ok(Some(spec)) => {
        ip.put_array("keys", spec.get_array("keys")); ip.put_string("stage", "frame");
        let it = ip.get_int("iters");
        push_frame(frames, &spec, shallow_obj(&in3), i, Data::DInt(it))
      }
      Ok(None) => match op_body(&op, shallow_obj(&in3)) {
        Err((c, m)) => err_ev(&mut f, &c, format!("op {}: {}", i, m), Some(i)),
        Ok((res, b)) => after_iter(&mut f, &op, i, res, b, false),
      },
    };
  }
  if stage == "closing" {
    return match closing_result(&op, &ip) {
      Err((c, m)) => err_ev(&mut f, &c, format!("op {}: {}", i, m), Some(i)),
      Ok((res, b, fin)) => {
        if ip.get_boolean("iterated") { let mut e = after_iter(&mut f, &op, i, res, b, fin); e.put_boolean("from_frame", true); e }
        else {
          let in1 = f.get_array("inputs").get_object(i);
          let mut e = op_ev(&f, &op, i, &in1); e.put_boolean("from_frame", true);
          let mut ffin = false; let rule = condition(&op, b, &mut ffin);
          finish_op(&mut f, e, i, res, b, rule, ffin, None)
        }
      }
    };
  }
  err_ev(&mut f, "panic", format!("op {} is in progress at stage '{}', which cannot step", i, stage), Some(i))
}
pub fn step_stack(frames: &mut DataArray, pick: &DataObject) -> DataObject {
  let n = frames.len();
  let mut f = frames.get_object(n - 1);
  let body = f.get_object("body");
  let phase = f.get_string("phase");
  if phase == "enter" {
    let mut e = ev("enter", &f);
    e.put_string("kind", &f.get_string("kind"));
    put_copy(&mut e, "args", f.get_property("args"));
    f.put_string("phase", "seed");
    return e;
  }
  if phase == "closing" {
    let oc = f.get_string("outcome");
    // A nested frame that closed LEAVES to the enclosing op with its outcome; `closing_result`
    // applies the per-outcome contract (spec 1.5.4 Frames / early exit): `terminate` completes the
    // op with the child's partial `out` and success TRUE (it does NOT end the parent); `fail`
    // becomes the op's FALSE condition (an empty result and b=false) so the op's own condition
    // decides; `ok` is a normal completion. A nested `next`-with-no-next-case (I-4) never reaches
    // here - the child's own `pending` arm ends it with an `error` event first (a fatal, not an
    // outcome), and `run` stops on that event (below). At the root, closing is `done`.
    if n > 1 { return leave(frames, &oc, DataArray::new()); }
    let mut e = ev("done", &f);
    e.put_string("outcome", &oc); put_copy(&mut e, "out", f.get_property("out")); e.put_boolean("fail", oc == "fail");
    f.put_string("phase", "ended");
    return e;
  }
  if phase == "ended" {
    // Terminal frame-stack invariant: `run` and every debugger `step` must end the run HERE, never
    // resume an ended frame. A nested frame that ended normally was already popped by `leave`, so
    // the only way to reach an ended frame as the TOP is (a) the run is over (the root ended) or
    // (b) the frames below are a residual stack left by an error/stall that `run` did not drain.
    // Either way the run is over: end the ROOT too so no sibling frame is left live.
    if n > 1 { frames.get_object(0).put_string("phase", "ended"); }
    return err_ev(&mut f, "ended", "the run has already ended".to_string(), None);
  }
  let cidx = f.get_int("case");
  let case = match case_at(&body, cidx) { Some(c) => c, None => { return err_ev(&mut f, "panic", format!("case {} is not in the chain", cidx), None); } };
  if phase == "seed" {
    let mut e = ev("seed", &f);
    let pre = seed_case(&case, &mut f);
    e.put_array("pre", pre);
    e.put_array("ready", ready_set(&case, &f));
    e.put_array("diag", diag_case(&case));
    f.put_string("phase", "run");
    return e;
  }
  // a fired rule's effect is the step after the eval that reports it
  if is_obj(&f, "pending") {
    let p = f.get_object("pending");
    let rule = p.get_string("rule"); let op = p.get_int("op");
    f.put_null("pending");
    if rule == "next" {
      if case.has("nextcase") {
        let mut e = ev("next", &f); e.put_int("op", op); e.put_int("from", cidx); e.put_int("to", cidx + 1);
        f.put_int("case", cidx + 1); f.put_string("phase", "seed");
        return e;
      }
      return err_ev(&mut f, "I-4", format!("a next rule fired on op {} in case {} but the chain has no next case (flowlang panics: nextcase.unwrap())", op, cidx), Some(op as usize));
    }
    if rule == "terminate" || rule == "fail" {
      let mut e = ev(&rule, &f); e.put_int("op", op);
      f.put_string("outcome", &rule); f.put_string("phase", "closing");
      return e;
    }
    let mut e = ev("finish", &f); e.put_int("op", op);
    f.put_boolean("finish", true);
    return e;
  }
  if is_obj(&f, "inprog") { return continue_op(frames, &case); }
  let chosen = if pick.has("kind") { Some((pick.get_string("kind"), pick.get_int("index") as usize)) } else { default_pick(&case, &mut f) };
  match chosen {
    None => { if n == 1 { end_of_case(&mut f) } else { end_nested(frames) } }
    Some((k, i)) => {
      let mut e = if k == "wire" { deliver(&case, &mut f, i) } else if k == "sync" { fire_sync(&case, &mut f, i) } else { start_op(frames, &case, i) };
      e.put_string("pick", if pick.has("kind") { "user" } else { "default" });
      e
    }
  }
}


/// The end state of a `run`: `out` is the root frame's shared result object and `outcome` is one of
/// "ok" (the chain completed or `terminate` fired) or "fail" (a `fail` rule, or an `error` event:
/// I-4, a stuck case, a depth overflow, or a caught panic).
pub struct StepRun {
  pub out: DataObject,
  pub outcome: String,
  pub event: DataObject,
  pub steps: i64,
  pub max_depth: i64,
}

/// Continue a frame stack to its end on the default pick: this IS `Code::execute`. The stack must
/// already hold a seeded root frame (see `new_frame`, phase "enter"). Runs `step_stack` with an
/// empty pick until the root frame's phase is "ended", then reads the outcome off it. Fast: no
/// events, ready sets, or state copies are produced here.
pub fn run(frames: &mut DataArray, budget: i64) -> StepRun {
  let max = if budget <= 0 { 1_000_000 } else { budget };
  let pick = DataObject::new();
  let mut steps = 0i64;
  let mut max_depth = frames.len() as i64;
  let mut last = DataObject::new();
  loop {
    let n = frames.len();
    if n == 0 {
      let mut e = DataObject::new();
      e.put_string("type", "error"); e.put_string("code", "panic"); e.put_string("msg", "the frame stack is empty");
      return StepRun { out: DataObject::new(), outcome: "fail".to_string(), event: e, steps, max_depth };
    }
    if frames.get_object(0).get_string("phase") == "ended" { break; }
    if steps >= max {
      let mut e = DataObject::new();
      e.put_string("type", "error"); e.put_string("code", "budget");
      e.put_string("msg", &format!("the flow did not finish within {} steps (stuck?)", max));
      return StepRun { out: frames.get_object(0).get_object("out"), outcome: "fail".to_string(), event: e, steps, max_depth };
    }
    let e = step_stack(frames, &pick);
    steps += 1;
    if (frames.len() as i64) > max_depth { max_depth = frames.len() as i64; }
    // Terminal-event break: a fatal event (error / stall) is terminal for the whole run even when
    // it was emitted from a NESTED frame that still has live siblings below it. `Code::execute`
    // always starts from a single root (such a nested error already ended the root), but a
    // debugger-driven `continue` (dev.flowstep, run with `run`) can hold a stack whose nested frame
    // errored while its ancestors are still open - without this break the loop steps a sibling
    // until the budget, which is exactly R1 reproduction (a). The event stays the PLAIN stepper
    // event; the terminal frame-stack invariant (step_stack's ended arm) ends the root next pass.
    let et = if e.has("type") { e.get_string("type") } else { String::new() };
    if frames.get_object(0).get_string("phase") != "ended" && (et == "error" || et == "stall") {
      let mut out = DataObject::new();
      out.put_string("outcome", "fail");
      return StepRun { out, outcome: "fail".to_string(), event: e, steps, max_depth };
    }
    last = e;
  }
  let root = frames.get_object(0);
  // The frame's outcome is set on the normal completion path (ok / terminate / fail via the
  // closing phase). A stall or an error event (I-4, a stuck case, depth, a caught panic) ends the
  // root without setting it, so read the outcome off the terminal event too.
  let mut outcome = { let o = root.get_string("outcome"); if o.is_empty() { "ok".to_string() } else { o } };
  let et = if last.has("type") { last.get_string("type") } else { String::new() };
  if outcome == "ok" && (et == "stall" || et == "error") { outcome = "fail".to_string(); }
  if et == "terminate" { outcome = "terminate".to_string(); }
  StepRun { out: root.get_object("out"), outcome, event: last, steps, max_depth }
}

/// Run a whole case chain from a JSON body to its end and report the terminal event: the
/// test/debugger view of `run`. Where `Code::execute` collapses every failure to
/// `Err(CodeException::Fail)`, this keeps the stepper's terminal event (type + code) so a caller
/// can tell a nested I-4 from a stall from a panic, and can see that a bounded run did NOT spend
/// its budget on a residual stepping loop (R1).
pub fn run_report(body: DataObject, args: DataObject, budget: i64) -> StepRun {
  let mut frames = DataArray::new();
  frames.push_object(new_frame(args, "root", body, DataArray::new()));
  run(&mut frames, budget)
}

