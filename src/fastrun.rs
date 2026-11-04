//! The fast path behind `Code::execute`: the stepper's default-pick run, without the stepper's
//! per-step bookkeeping.
//!
//! `crate::stepper` defines flow execution one step at a time. The debugger (newbound's
//! dev.flowstep) needs that: every step emits an event, and the whole state is ndata, so it
//! can be saved, copied and resumed. An ordinary call needs only the result. Running it through
//! `step_stack` costs about 13x the pre-stepper interpreter: every flag, cursor and index is a
//! string-keyed ndata lookup, and every step builds an event with deep copies that is then
//! dropped and garbage-collected.
//!
//! This module runs the same flow with native bookkeeping. It is a second implementation of
//! the stepper's continue-to-end on the default pick, and the stepper is its oracle:
//! - It takes the same steps in the same order. It counts them the same way (one per stepper
//!   event), so the 1,000,000-step budget ends the same run at the same point.
//! - It produces the same result, side effects, error codes and messages. Where flowlang
//!   panics, it panics in the same place.
//! - `tests/fastrun_parity.rs` checks all of this, including step counts, against
//!   `stepper::run_report` on fixtures and on randomly generated flows. Any change to the
//!   stepper's semantics must be made here too, and that test catches a miss.
//!
//! Values are never held only in Rust. ndata's `Data::DObject`/`DArray` are uncounted
//! references, and gc is global, so another thread's gc could free a value that no ndata
//! container refers to. Every value therefore lives in an ndata container this run holds a
//! handle to (each op's inputs, its result, the frame's args and out, an iteration's
//! containers), as in the stepper. Only flags, counters, cursors and indices are native.
//!
//! Commands that op nodes call are loaded once per top-level run (a per-run cache), not once
//! per evaluation. So within one run, an edit to a callee takes effect from the next run.

use std::collections::{HashMap, HashSet};
use std::panic;
use std::rc::Rc;
use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use ndata::databytes::DataBytes;
use ndata::data::Data;
use crate::case::Case;
use crate::code::CodeException;
use crate::command::{Command, Source};
use crate::datastore::DataStore;
use crate::primitives::Primitive;
use crate::stepper::{match_flag, panic_text};

/// The step budget `Code::execute` runs under (the same cap as `stepper::run` with budget <= 0).
pub const DEFAULT_BUDGET: i64 = 1_000_000;
/// Frames nest at most this deep (root included), as in the stepper's `push_frame`.
const MAX_DEPTH: usize = 64;
/// Collect ndata garbage every this many steps, so a long run's garbage does not pile up until
/// it returns (and one huge collection does not hold the global heap locks for long).
const GC_EVERY: i64 = 1024;

// ---------------------------------------------------------------------------------------------
// The compiled graph: a Case with its names resolved to indices once per run.

#[derive(PartialEq)]
enum OpType { Primitive, Command, Local, Constant, Persistent, Match, Undefined, Other }

struct COp {
  typ: OpType,
  name: String,
  ctype: Option<String>,
  cmd: Option<String>,
  localdata: Option<Box<CCase>>,
  cond: Option<(bool, String)>,
  /// input names; a slot is an index into this
  ins: Vec<String>,
  /// whether each input is pre-delivered (null) at seed: its name is no wire's dest name in the case
  pre: Vec<bool>,
  outs: Vec<String>,
  list_in: Vec<String>,
  list_out: Vec<String>,
  /// (output, the input it feeds), or the I-8 fault `modes` raises when an op starts
  loop_out: Result<Vec<(String, String)>, String>,
  /// sources of this op's valid incoming synchros
  in_syncs: Vec<usize>,
  prim: Option<crate::rustcmd::Transform>,
}

struct CCon { si: i64, sn: String, di: i64, dn: String, slot: Option<usize> }

struct CCase {
  ops: Vec<COp>,
  cons: Vec<CCon>,
  /// (src, dest, valid)
  syncs: Vec<(i64, i64, bool)>,
  /// the return bar's names: a flow callee's results map onto a command op's outputs in this order
  out_keys: Vec<String>,
  next: Option<Box<CCase>>,
}

fn sync_valid(s: i64, d: i64, n: usize) -> bool { s >= 0 && d >= 0 && s != d && (s as usize) < n && (d as usize) < n }

fn compile(c: &Case) -> CCase {
  let n = c.cmds.len();
  let dest_names: HashSet<&str> = c.cons.iter().map(|x| x.dest.name.as_str()).collect();
  let mut ops = Vec::with_capacity(n);
  for (i, op) in c.cmds.iter().enumerate() {
    let ins: Vec<String> = op.input.keys().cloned().collect();
    let pre = ins.iter().map(|k| !dest_names.contains(k.as_str())).collect();
    let outs: Vec<String> = op.output.keys().cloned().collect();
    let list_in = op.input.iter().filter(|(_, nd)| nd.mode == "list").map(|(k, _)| k.clone()).collect();
    let mut list_out = Vec::new();
    let mut loop_out: Result<Vec<(String, String)>, String> = Ok(Vec::new());
    for (k, nd) in &op.output {
      if nd.mode == "list" { list_out.push(k.clone()); }
      else if nd.mode == "loop" {
        match (&nd.looop, &mut loop_out) {
          (Some(l), Ok(v)) => v.push((k.clone(), l.clone())),
          (None, Ok(_)) => loop_out = Err(format!("loop output '{}' names no input (I-8)", k)),
          _ => {}
        }
      }
    }
    let typ = match op.cmd_type.as_str() {
      "primitive" => OpType::Primitive, "command" => OpType::Command, "local" => OpType::Local,
      "constant" => OpType::Constant, "persistent" => OpType::Persistent, "match" => OpType::Match,
      "undefined" => OpType::Undefined, _ => OpType::Other,
    };
    let prim = if typ == OpType::Primitive { Primitive::find(&op.name) } else { None };
    let in_syncs = c.syncs.iter()
      .filter(|s| s.dest == i as i64 && sync_valid(s.src, s.dest, n))
      .map(|s| s.src as usize).collect();
    ops.push(COp {
      typ, name: op.name.clone(), ctype: op.ctype.clone(), cmd: op.cmd.clone(),
      localdata: op.localdata.as_ref().map(|l| Box::new(compile(l))),
      cond: op.condition.as_ref().map(|c| (c.value, c.rule.clone())),
      ins, pre, outs, list_in, list_out, loop_out, in_syncs, prim,
    });
  }
  let cons = c.cons.iter().map(|x| {
    let (di, dn) = (x.dest.index, x.dest.name.clone());
    let slot = if di >= 0 && (di as usize) < n { ops[di as usize].ins.iter().position(|k| *k == dn) } else { None };
    CCon { si: x.src.index, sn: x.src.name.clone(), di, dn, slot }
  }).collect();
  let syncs = c.syncs.iter().map(|s| (s.src, s.dest, sync_valid(s.src, s.dest, n))).collect();
  CCase {
    ops, cons, syncs,
    out_keys: c.output.keys().cloned().collect(),
    next: c.nextcase.as_ref().map(|nx| Box::new(compile(nx))),
  }
}

// ---------------------------------------------------------------------------------------------
// Run state

/// Why a run cannot continue (the stepper's `stall` and `error` events). Either ends the whole run.
enum Fatal { Stall, Error(String, String) }
fn fatal(code: &str, msg: String) -> Fatal { Fatal::Error(code.to_string(), msg) }
type Fault = (String, String);
fn fault(code: &str, msg: String) -> Fault { (code.to_string(), msg) }
fn at_op(i: usize) -> impl Fn(Fault) -> Fatal { move |(c, m)| Fatal::Error(c, format!("op {}: {}", i, m)) }

#[derive(PartialEq, Clone, Copy)]
enum Oc { Ok, Terminate, Fail }

/// How a frame ended: its outcome, its `out`, and its finish flag.
struct FrameEnd { oc: Oc, out: DataObject, finish: bool }

/// A command op's callee, loaded once per run.
#[derive(Clone)]
enum Loaded { Failed(String), Native(Rc<Command>), Flow(Rc<CCase>) }

struct Ctx { steps: i64, max: i64, cache: HashMap<String, Loaded> }

impl Ctx {
  /// Take one step (one stepper event), or end the run when the budget is spent.
  /// Called BEFORE the step's work, as `stepper::run` checks the budget before `step_stack`.
  #[inline]
  fn step(&mut self) -> Result<(), Fatal> {
    if self.steps >= self.max {
      return Err(fatal("budget", format!("the flow did not finish within {} steps (stuck?)", self.max)));
    }
    self.steps += 1;
    // At a step boundary every live value is held by an ndata container this run has a handle
    // to (see the module doc), so collecting here cannot free anything the run still uses.
    if self.steps % GC_EVERY == 0 { DataObject::gc(); DataArray::gc(); DataBytes::gc(); }
    Ok(())
  }

  fn load(&mut self, cmdstr: &str, lib: &str, id: &str) -> Loaded {
    if let Some(l) = self.cache.get(cmdstr) { return l.clone(); }
    let (l2, i2) = (lib.to_string(), id.to_string());
    let l = match panic::catch_unwind(panic::AssertUnwindSafe(move || Command::new(&l2, &i2))) {
      Err(p) => Loaded::Failed(panic_text(&p)),
      Ok(c) => {
        if c.lang == "flow" {
          match &c.src { Source::Flow(case) => Loaded::Flow(Rc::new(compile(case))), _ => Loaded::Native(Rc::new(c)) }
        } else { Loaded::Native(Rc::new(c)) }
      }
    };
    self.cache.insert(cmdstr.to_string(), l.clone());
    l
  }
}

/// One case's state: the stepper's per-case frame bookkeeping, with native flags and indices.
struct St {
  wires: Vec<bool>,
  syncs: Vec<bool>,
  done: Vec<bool>,
  /// per op: delivered inputs (values live in ndata) and which slots are delivered
  inputs: Vec<DataObject>,
  dlv: Vec<Vec<bool>>,
  ndlv: Vec<usize>,
  results: Vec<Option<DataObject>>,
  prio: Vec<usize>,
  cur_wire: bool,
  cur_k: usize,
  pending: Option<(String, usize)>,
}

impl St {
  fn seed(c: &CCase) -> St {
    let n = c.ops.len();
    let mut inputs = Vec::with_capacity(n);
    let mut dlv = Vec::with_capacity(n);
    let mut ndlv = Vec::with_capacity(n);
    for op in &c.ops {
      let mut iv = DataObject::new();
      let mut cnt = 0;
      for (j, k) in op.ins.iter().enumerate() { if op.pre[j] { iv.put_null(k); cnt += 1; } }
      inputs.push(iv); dlv.push(op.pre.clone()); ndlv.push(cnt);
    }
    let mut st = St {
      wires: vec![false; c.cons.len()], syncs: vec![false; c.syncs.len()], done: vec![false; n],
      inputs, dlv, ndlv, results: vec![None; n], prio: Vec::new(),
      cur_wire: true, cur_k: 0, pending: None,
    };
    // case-start rule: no awaited input AND no incoming synchro, in index order
    st.prio = (0..n).filter(|&i| st.op_ready(c, i)).collect();
    st
  }

  #[inline]
  fn op_ready(&self, c: &CCase, i: usize) -> bool {
    !self.done[i] && self.ndlv[i] == c.ops[i].ins.len() && c.ops[i].in_syncs.iter().all(|&s| self.done[s])
  }
  #[inline]
  fn wire_ready(&self, c: &CCase, k: usize) -> bool {
    if self.wires[k] { return false; }
    let si = c.cons[k].si;
    if si == -1 || si < 0 || si as usize >= c.ops.len() { return true; }
    self.done[si as usize]
  }
  #[inline]
  fn sync_ready(&self, c: &CCase, k: usize) -> bool {
    if self.syncs[k] { return false; }
    let (a, _, valid) = c.syncs[k];
    if !valid { return true; }
    self.done[a as usize]
  }
  fn prio_front(&mut self, i: usize) { self.prio.retain(|&x| x != i); self.prio.insert(0, i); }

  /// The stepper's `default_pick`: case-start/ready ops first, then wire and synchro passes from the cursor.
  fn default_pick(&mut self, c: &CCase) -> Option<Pick> {
    let mut chosen = None;
    let mut keep = Vec::new();
    for idx in 0..self.prio.len() {
      let i = self.prio[idx];
      if self.op_ready(c, i) { if chosen.is_none() { chosen = Some(i); } else { keep.push(i); } }
    }
    self.prio = keep;
    if let Some(i) = chosen { return Some(Pick::Op(i)); }
    let (nw, ns) = (c.cons.len(), c.syncs.len());
    let mut pass_wire = self.cur_wire;
    let mut k = self.cur_k;
    for _round in 0..3 {
      if pass_wire {
        for j in k..nw { if self.wire_ready(c, j) { self.cur_wire = true; self.cur_k = j + 1; return Some(Pick::Wire(j)); } }
        pass_wire = false; k = 0;
      } else {
        for j in k..ns { if self.sync_ready(c, j) { self.cur_wire = false; self.cur_k = j + 1; return Some(Pick::Sync(j)); } }
        pass_wire = true; k = 0;
      }
    }
    self.cur_wire = pass_wire; self.cur_k = k;
    (0..c.ops.len()).find(|&i| self.op_ready(c, i)).map(Pick::Op)
  }

  /// A wire fires (the stepper's `deliver`).
  fn deliver(&mut self, c: &CCase, k: usize, args: &DataObject, out: &mut DataObject) -> Result<(), Fatal> {
    let n = c.ops.len();
    let con = &c.cons[k];
    let (si, di) = (con.si, con.di);
    let val: Data;
    if si == -1 { val = if args.has(&con.sn) { args.get_property(&con.sn) } else { Data::DNull }; }
    else if si < 0 || si as usize >= n { return Err(fatal("panic", format!("wire {} has src index {} out of range", k, si))); }
    else {
      match &self.results[si as usize] {
        Some(r) if r.has(&con.sn) => { val = r.get_property(&con.sn); }
        _ => return Err(fatal("absent-output", format!("wire {} reads output '{}' of op {}, which its result lacks (flowlang panics in get_property)", k, con.sn, si))),
      }
    }
    if di != -2 && (di < 0 || di as usize >= n) { return Err(fatal("panic", format!("wire {} has dest index {} out of range", k, di))); }
    self.wires[k] = true;
    if di == -2 { out.set_property(&con.dn, val); return Ok(()); }
    let du = di as usize;
    if c.ops[du].typ == OpType::Undefined { self.done[du] = true; self.results[du] = None; return Ok(()); }
    let slot = match con.slot {
      Some(s) => s,
      None => return Err(fatal("panic", format!("wire {} targets input '{}', which op {} does not have", k, con.dn, du))),
    };
    if self.done[du] { return Ok(()); } // a second delivery does not re-run the op (I-1)
    self.inputs[du].set_property(&con.dn, val);
    if !self.dlv[du][slot] { self.dlv[du][slot] = true; self.ndlv[du] += 1; }
    if self.op_ready(c, du) { self.prio_front(du); }
    Ok(())
  }

  fn fire_sync(&mut self, c: &CCase, k: usize) {
    self.syncs[k] = true;
    let (_, b, valid) = c.syncs[k];
    if valid && self.op_ready(c, b as usize) { self.prio_front(b as usize); }
  }

  /// The op's final eval: done unless next/terminate/fail fired; a fired rule's effect is the next step.
  fn finish_op(&mut self, i: usize, out: DataObject, rule: Option<String>, ffin: bool, frame_finish: &mut bool) {
    let done = !matches!(rule.as_deref(), Some("next") | Some("terminate") | Some("fail"));
    if done { self.done[i] = true; self.results[i] = Some(out); }
    if ffin && rule.as_deref() != Some("finish") { *frame_finish = true; }
    if let Some(r) = rule { self.pending = Some((r, i)); }
  }
}

enum Pick { Wire(usize), Sync(usize), Op(usize) }

// ---------------------------------------------------------------------------------------------
// Ops


/// The op's condition; a finish rule marks `ffin` (the enclosing frame's flag).
fn condition(op: &COp, b: bool, ffin: &mut bool) -> Option<String> {
  if op.typ == OpType::Constant { return None; }
  let (value, rule) = match &op.cond { Some(c) => c, None => return None };
  if *value != b { return None; }
  match rule.as_str() {
    "next" | "terminate" | "fail" => Some(rule.clone()),
    "finish" => { *ffin = true; Some(rule.clone()) }
    _ => None,
  }
}

/// What a frame-opening op runs: a local's body, or a flow callee (with its return-bar keys).
enum Spec<'a> { Local(&'a CCase), Callee(Rc<CCase>) }
/// How an op evaluates: it opens a frame, calls a native command, or is evaluated in place.
enum Body<'a> { Frame(Spec<'a>), Native(Rc<Command>), Plain }

fn split_cmd(op: &COp) -> Result<(String, String, String), Fault> {
  let cmdstr = match &op.cmd { Some(c) => c.clone(), None => return Err(fault("panic", "command op has no cmd".to_string())) };
  let sa: Vec<&str> = cmdstr.split(':').collect();
  if sa.len() < 3 { return Err(fault("panic", format!("command op cmd '{}' is not lib:ctl:cmd", cmdstr))); }
  let (lib, id) = (sa[0].to_string(), sa[2].to_string());
  Ok((cmdstr, lib, id))
}

/// The stepper's `frame_spec`, plus the native callee its `op_body` would load.
fn resolve<'a>(ctx: &mut Ctx, op: &'a COp) -> Result<Body<'a>, Fault> {
  match op.typ {
    OpType::Local => Ok(match &op.localdata { Some(l) => Body::Frame(Spec::Local(l)), None => Body::Plain }),
    OpType::Command => {
      let (cmdstr, lib, id) = split_cmd(op)?;
      match ctx.load(&cmdstr, &lib, &id) {
        Loaded::Failed(t) => Err(fault("panic", format!("command '{}' could not be loaded: {}", cmdstr, t))),
        Loaded::Flow(cc) => Ok(Body::Frame(Spec::Callee(cc))),
        Loaded::Native(c) => Ok(Body::Native(c)),
      }
    }
    _ => Ok(Body::Plain),
  }
}

/// One evaluation of an op body that opens no frame (the stepper's `op_body`).
fn op_body(op: &COp, body: Body, in1: DataObject) -> Result<(DataObject, bool), Fault> {
  let v = &op.name;
  let mut out = DataObject::new();
  let mut b = true;
  match op.typ {
    OpType::Primitive => {
      let f = match op.prim { Some(f) => f, None => return Err(fault("panic", format!("primitive '{}' panicked: No such primitive {}", v, v))) };
      match panic::catch_unwind(panic::AssertUnwindSafe(move || f(in1))) {
        Ok(o) => { out = o; }
        Err(p) => return Err(fault("panic", format!("primitive '{}' panicked: {}", v, panic_text(&p)))),
      }
    }
    OpType::Local => { for k in &op.outs { out.put_null(k); } }
    OpType::Constant => {
      let ctype = match &op.ctype { Some(c) => c.as_str(), None => return Err(fault("panic", "constant op has no ctype".to_string())) };
      for k in &op.outs {
        let bad = |e: String| fault("panic", format!("constant '{}' does not parse as {}: {}", v, ctype, e));
        match ctype {
          "int" => match v.parse::<i64>() { Ok(x) => out.put_int(k, x), Err(e) => return Err(bad(e.to_string())) },
          "decimal" => match v.parse::<f64>() { Ok(x) => out.put_float(k, x), Err(e) => return Err(bad(e.to_string())) },
          "boolean" => match v.parse::<bool>() { Ok(x) => out.put_boolean(k, x), Err(e) => return Err(bad(e.to_string())) },
          "string" => match DataObject::try_from_string(&format!("{{\"v\":\"{}\"}}", v)) { Ok(o) => out.put_string(k, &o.get_string("v")), Err(_) => return Err(bad("bad string literal".to_string())) },
          "object" => { let vv = v.clone(); match panic::catch_unwind(panic::AssertUnwindSafe(move || DataObject::from_string(&vv))) { Ok(o) => out.put_object(k, o), Err(p) => return Err(bad(panic_text(&p))) } },
          "array" => { let vv = v.clone(); match panic::catch_unwind(panic::AssertUnwindSafe(move || DataArray::from_string(&vv))) { Ok(o) => out.put_array(k, o), Err(p) => return Err(bad(panic_text(&p))) } },
          _ => out.put_null(k),
        }
      }
    }
    OpType::Command => {
      let sub = match body {
        Body::Native(c) => c,
        _ => return Err(fault("panic", "command op has no native callee".to_string())),
      };
      let cmdstr = op.cmd.clone().unwrap_or_default();
      let mut params = DataObject::new();
      for (k, d) in in1.objects() { params.set_property(&k, d); }
      let s2 = &*sub; let p2 = params.clone();
      let res = match panic::catch_unwind(panic::AssertUnwindSafe(move || s2.execute(p2))) {
        Ok(Ok(o)) => Some(o),
        Ok(Err(_)) => None,
        Err(p) => return Err(fault("panic", format!("command '{}' panicked: {}", cmdstr, panic_text(&p)))),
      };
      match res {
        None => { b = false; }
        Some(r) => {
          for (i, key1) in op.outs.iter().enumerate() {
            if i >= 1 { return Err(fault("absent-output", format!("command op output '{}' has no positional counterpart among the callee's {} output(s) (I-7)", key1, 1))); }
            if !r.has("a") { return Err(fault("absent-output", "the callee's result lacks 'a' (flowlang panics in get_property)".to_string())); }
            out.set_property(key1, r.get_property("a"));
          }
        }
      }
    }
    OpType::Persistent => {
      let mut g = DataStore::globals();
      let ks = in1.clone().keys();
      if ks.len() > 0 { g.set_property(v, in1.get_property(&ks[0])); }
      let value = if g.has(v) { g.get_property(v) } else { Data::DNull };
      for k in &op.outs { out.set_property(k, value.clone()); }
    }
    OpType::Match => {
      let ks = in1.clone().keys();
      if ks.is_empty() { return Err(fault("panic", "a match op needs one input (I-5)".to_string())); }
      let ctype = match &op.ctype { Some(c) => c, None => return Err(fault("panic", "match op has no ctype".to_string())) };
      b = match_flag(ctype, v, in1.get_property(&ks[0]))?;
    }
    // undefined and unknown types yield {}
    OpType::Undefined | OpType::Other => {}
  }
  Ok((out, b))
}

/// A closed frame's result as the op's result: fail = b false + EMPTY result; a command maps by position (I-7).
fn closing_result(op: &COp, spec: &Spec, child: FrameEnd) -> Result<(DataObject, bool, bool), Fault> {
  if child.oc == Oc::Fail { return Ok((DataObject::new(), false, false)); }
  let keys = match spec {
    Spec::Local(_) => return Ok((child.out, true, child.finish)),
    Spec::Callee(cc) => &cc.out_keys,
  };
  let mut out = DataObject::new();
  for (i, key1) in op.outs.iter().enumerate() {
    if i >= keys.len() { return Err(fault("absent-output", format!("command op output '{}' has no positional counterpart among the callee's {} output(s) (I-7)", key1, keys.len()))); }
    let key2 = &keys[i];
    if !child.out.has(key2) { return Err(fault("absent-output", format!("the callee's result lacks '{}' (flowlang panics in get_property)", key2))); }
    out.set_property(key1, child.out.get_property(key2));
  }
  Ok((out, true, false))
}

fn list_count(in1: &DataObject, list_in: &[String]) -> Result<usize, Fault> {
  let mut count = 0usize;
  for (j, k) in list_in.iter().enumerate() {
    let d = if in1.has(k) { in1.get_property(k) } else { Data::DNull };
    if !d.is_array() { return Err(fault("panic", format!("list input '{}' is not an array", k))); }
    let l = d.array().len();
    if j == 0 || l < count { count = l; }
  }
  Ok(count)
}

fn build_in3(in1: &DataObject, list_in: &[String], i: usize) -> Result<DataObject, Fault> {
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

/// A list/loop op's iteration state (the stepper's `inprog` for an iterated op).
struct Iter {
  idx: usize, iters: i64, nl: usize, count: usize,
  in1: DataObject, out3: DataObject,
  lastb: bool, last_rule: Option<String>, ofin: bool, ffin: bool,
}

/// One iteration's result (the stepper's `after_iter`). Ok(true) when the op finished.
fn after_iter(st: &mut St, op: &COp, loop_out: &[(String, String)], i: usize, it: &mut Iter, res: DataObject, b: bool, fin: bool, frame_finish: &mut bool) -> Result<bool, Fatal> {
  let iter_no = it.iters;
  it.iters += 1;
  if fin { it.ofin = true; }
  let mut ffin = it.ffin;
  let rule = condition(op, b, &mut ffin);
  it.ffin = ffin;
  it.lastb = b;
  if matches!(rule.as_deref(), Some("next") | Some("terminate") | Some("fail")) {
    st.finish_op(i, res, rule, ffin, frame_finish);
    return Ok(true);
  }
  if rule.is_some() { it.last_rule = Some("finish".to_string()); }
  for k in &op.outs {
    if !res.has(k) { return Err(fatal("absent-output", format!("op {}: iteration {} result lacks output '{}' (flowlang panics in get_property)", i, iter_no, k))); }
    let dp = res.get_property(k);
    if op.list_out.contains(k) { it.out3.get_array(k).push_property(dp); }
    else {
      it.out3.set_property(k, dp.clone());
      if let Some((_, lf)) = loop_out.iter().find(|(a, _)| a == k) { it.in1.set_property(lf, dp); }
    }
  }
  let mut last = it.ofin;
  if !last && it.nl > 0 { it.idx += 1; if it.idx == it.count { last = true; } }
  if last {
    st.finish_op(i, it.out3.clone(), it.last_rule.clone(), ffin, frame_finish);
    return Ok(true);
  }
  Ok(false)
}

/// Run op `i` to completion (the stepper's `start_op` and every sub-step of its op in progress).
fn run_op(ctx: &mut Ctx, c: &CCase, st: &mut St, i: usize, frame_finish: &mut bool, depth: usize) -> Result<(), Fatal> {
  let op = &c.ops[i];
  ctx.step()?; // start_op
  st.prio.retain(|&x| x != i);
  // The stepper copies the op's inputs here; the container is never read again once the op
  // starts (a later delivery to it is ignored, and next/terminate/fail end the case), so it is
  // handed over instead.
  let in1 = std::mem::replace(&mut st.inputs[i], DataObject::new());
  let loop_out: &[(String, String)] = match &op.loop_out { Ok(v) => v, Err(m) => return Err(fatal("panic", format!("op {}: {}", i, m))) };
  if op.list_in.is_empty() && loop_out.is_empty() {
    let (res, b) = match resolve(ctx, op).map_err(at_op(i))? {
      Body::Frame(spec) => {
        if depth >= MAX_DEPTH { return Err(fatal("depth", "frames nested more than 64 deep (recursive flow?)".to_string())); }
        let child = run_frame(ctx, spec_case(&spec), in1, depth + 1)?;
        ctx.step()?; // the op's closing eval
        let (res, b, _) = closing_result(op, &spec, child).map_err(at_op(i))?;
        (res, b)
      }
      body => op_body(op, body, in1).map_err(at_op(i))?,
    };
    let mut ffin = false;
    let rule = condition(op, b, &mut ffin);
    st.finish_op(i, res, rule, ffin, frame_finish);
    return Ok(());
  }
  let count = list_count(&in1, &op.list_in).map_err(at_op(i))?;
  let mut out3 = DataObject::new();
  for k in &op.list_out { out3.put_array(k, DataArray::new()); }
  if loop_out.is_empty() && count == 0 {
    // zero iterations: no body, no condition, list outputs [], the others null
    for k in &op.outs { if !out3.has(k) { out3.put_null(k); } }
    st.finish_op(i, out3, None, false, frame_finish);
    return Ok(());
  }
  let mut it = Iter {
    idx: 0, iters: 0, nl: op.list_in.len(), count, in1, out3,
    lastb: true, last_rule: None, ofin: false, ffin: false,
  };
  // each iteration's in3 is fresh and is not read after its body, so the body gets it as is
  let mut in3 = build_in3(&it.in1, &op.list_in, 0).map_err(at_op(i))?; // the first iter (same step)
  loop {
    ctx.step()?; // the iteration's body
    let (res, b, fin) = match resolve(ctx, op).map_err(at_op(i))? {
      Body::Frame(spec) => {
        if depth >= MAX_DEPTH { return Err(fatal("depth", "frames nested more than 64 deep (recursive flow?)".to_string())); }
        let child = run_frame(ctx, spec_case(&spec), in3, depth + 1)?;
        ctx.step()?; // the iteration's closing eval
        closing_result(op, &spec, child).map_err(at_op(i))?
      }
      body => { let (r, b) = op_body(op, body, in3).map_err(at_op(i))?; (r, b, false) }
    };
    if after_iter(st, op, loop_out, i, &mut it, res, b, fin, frame_finish)? { return Ok(()); }
    ctx.step()?; // the next iter
    in3 = build_in3(&it.in1, &op.list_in, it.idx).map_err(at_op(i))?;
  }
}

fn spec_case<'s>(spec: &'s Spec) -> &'s CCase { match spec { Spec::Local(c) => c, Spec::Callee(cc) => cc } }

/// Run one frame (a case chain) to its end. The frame's `enter` step is the caller's.
fn run_frame(ctx: &mut Ctx, chain: &CCase, args: DataObject, depth: usize) -> Result<FrameEnd, Fatal> {
  let mut out = DataObject::new();
  let mut finish = false;
  let mut case = chain;
  let mut cidx = 0;
  'cases: loop {
    ctx.step()?; // seed
    let mut st = St::seed(case);
    loop {
      if let Some((rule, op)) = st.pending.take() {
        ctx.step()?; // the rule's effect
        match rule.as_str() {
          "next" => {
            if let Some(nx) = &case.next { case = nx; cidx += 1; continue 'cases; }
            return Err(fatal("I-4", format!("a next rule fired on op {} in case {} but the chain has no next case (flowlang panics: nextcase.unwrap())", op, cidx)));
          }
          "terminate" | "fail" => {
            ctx.step()?; // closing: leave (nested) or done (root)
            let oc = if rule == "fail" { Oc::Fail } else { Oc::Terminate };
            return Ok(FrameEnd { oc, out, finish });
          }
          _ => { finish = true; continue; }
        }
      }
      match st.default_pick(case) {
        None => {
          ctx.step()?; // end of case: done/leave, or stall
          if st.wires.iter().any(|w| !w) || st.syncs.iter().any(|s| !s) { return Err(Fatal::Stall); }
          return Ok(FrameEnd { oc: Oc::Ok, out, finish });
        }
        Some(Pick::Wire(k)) => { ctx.step()?; st.deliver(case, k, &args, &mut out)?; }
        Some(Pick::Sync(k)) => { ctx.step()?; st.fire_sync(case, k); }
        Some(Pick::Op(i)) => { run_op(ctx, case, &mut st, i, &mut finish, depth)?; }
      }
    }
  }
}

// ---------------------------------------------------------------------------------------------
// Entry points

/// The end of a fast run, in `stepper::StepRun`'s terms: `outcome` is "ok", "terminate" or
/// "fail"; `code` is the stepper's terminal code for a failed run ("fail" for a fail rule,
/// "stall", or an error event's code such as "I-4", "absent-output", "panic", "depth" or "budget").
pub struct FastRun {
  pub out: DataObject,
  pub outcome: String,
  pub code: String,
  pub msg: String,
  pub steps: i64,
  pub finish: bool,
}

/// Run a case chain to its end on the default pick under `budget` steps (<= 0: DEFAULT_BUDGET).
/// The same run as `stepper::run_report(case.to_data(), args, budget)`.
pub fn run_report(case: &Case, args: DataObject, budget: i64) -> FastRun {
  let max = if budget <= 0 { DEFAULT_BUDGET } else { budget };
  let mut ctx = Ctx { steps: 0, max, cache: HashMap::new() };
  let chain = compile(case);
  let r = ctx.step().and_then(|_| run_frame(&mut ctx, &chain, args, 1)); // the root's enter, then its frame
  match r {
    Ok(fe) => {
      let (outcome, code) = match fe.oc { Oc::Ok => ("ok", ""), Oc::Terminate => ("terminate", ""), Oc::Fail => ("fail", "fail") };
      FastRun { out: fe.out, outcome: outcome.to_string(), code: code.to_string(), msg: String::new(), steps: ctx.steps, finish: fe.finish }
    }
    Err(Fatal::Stall) => FastRun { out: DataObject::new(), outcome: "fail".to_string(), code: "stall".to_string(), msg: String::new(), steps: ctx.steps, finish: false },
    Err(Fatal::Error(code, msg)) => FastRun { out: DataObject::new(), outcome: "fail".to_string(), code, msg, steps: ctx.steps, finish: false },
  }
}

/// `Code::execute`'s run: the result, and the root frame's finish flag.
pub fn execute(case: &Case, args: DataObject) -> (Result<DataObject, CodeException>, bool) {
  let r = run_report(case, args, 0);
  let res = match r.outcome.as_str() {
    "fail" => {
      match r.code.as_str() {
        "fail" | "stall" | "I-4" => {}
        _ => println!("FLOW ERROR [{}]: {}", r.code, r.msg),
      }
      Err(CodeException::Fail)
    }
    _ => Ok(r.out),
  };
  (res, r.finish)
}
