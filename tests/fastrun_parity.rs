//! `Code::execute` runs flows through `flowlang::fastrun`, a native re-implementation of the
//! stepper's continue-to-end on the default pick. The stepper (`flowlang::stepper`) is the
//! definition, and this test is its oracle check: every flow here runs through both
//! `stepper::run_report` and `fastrun::run_report`, and the two must agree on:
//! - the outcome, the terminal code and its message;
//! - where an error or a stall happened (the frame path and the case), and what a stall left
//!   unfired;
//! - the exact number of steps (so the step budget cuts both runs at the same point);
//! - the result;
//! - every persistent global the flow can touch, and the args after the run (primitives
//!   mutate shared values in place).
//!
//! The flows are the repository's testflow commands plus a few thousand randomly generated
//! bodies. The generator covers every op type, list/loop modes, conditions with every rule,
//! synchros (valid and inert), case chains, nested locals, command ops calling flow and Rust
//! callees, and broken graphs: fan-in, unwired inputs that share a wired name, cycles, indices
//! out of range, absent outputs, bad literals, unknown primitives and op types, and open loops
//! that only the budget ends.
//!
//! The generator stays away from what is random in BOTH implementations, because two runs of
//! it would not agree even with the same implementation: the order of an ndata object's keys
//! (`keys`, `to_json`, `to_string` and `set` could expose it), a command op with more than one
//! output (mapped by position over unordered keys, I-7), and a persistent or match op with
//! more than one input. It also never feeds a loop output into a list input, because that
//! panics flowlang outside any guard, and never builds cyclic data (no `push`).
//!
//! One #[test] on purpose: the store and ndata globals initialize once per process.

use flowlang::case::Case;
use flowlang::code::Code;
use flowlang::datastore::DataStore;
use flowlang::fastrun;
use flowlang::stepper;
use ndata::data::Data;
use ndata::dataarray::DataArray;
use ndata::dataobject::DataObject;

// ---- canonical JSON (sorted keys), so results compare independent of key order ----

fn canon(d: &Data) -> String {
  match d {
    Data::DObject(_) => {
      let o = d.object();
      let mut ks = o.clone().keys();
      ks.sort();
      let parts: Vec<String> = ks.iter().map(|k| format!("{:?}:{}", k, canon(&o.get_property(k)))).collect();
      format!("{{{}}}", parts.join(","))
    }
    Data::DArray(_) => {
      let a = d.array();
      let parts: Vec<String> = a.objects().iter().map(canon).collect();
      format!("[{}]", parts.join(","))
    }
    Data::DString(s) => format!("{:?}", s),
    Data::DInt(i) => format!("{}", i),
    Data::DFloat(f) => format!("{:?}", f),
    Data::DBoolean(b) => format!("{}", b),
    Data::DNull => "null".to_string(),
    _ => "<bytes>".to_string(),
  }
}
fn canon_obj(o: &DataObject) -> String { canon(&Data::DObject(o.data_ref)) }

// ---- the two runs ----

const GLOBALS: [&str; 3] = ["fr_g0", "fr_g1", "fr_g2"];

fn preset_globals() {
  let mut g = DataStore::globals();
  g.put_string("fr_g0", "previous");
  g.put_int("fr_g1", 7);
  g.remove_property("fr_g2");
}
fn snapshot_globals() -> String {
  let g = DataStore::globals();
  GLOBALS.iter().map(|k| if g.has(k) { canon(&g.get_property(k)) } else { "<unset>".to_string() }).collect::<Vec<_>>().join("|")
}

/// `place` is where an error or stall happened: the frame path (each frame-opening op and its
/// iteration) and the case of the frame it happened in, from the stepper's terminal event and
/// from the fast path's FlowError. `stall` is a stuck case's unfired wires, unfired synchros
/// and ops that never ran.
struct Seen { outcome: String, code: String, msg: String, place: String, stall: String, steps: i64, out: String, globals: String, args: String }

fn via_stepper(case: &Case, args: &DataObject, budget: i64) -> Seen {
  preset_globals();
  let a = args.deep_copy();
  let r = stepper::run_report(case.to_data(), a.clone(), budget);
  let e = &r.event;
  let et = if e.has("type") { e.get_string("type") } else { String::new() };
  let (code, msg) = if r.outcome != "fail" { (String::new(), String::new()) }
    else if et == "error" { (e.get_string("code"), e.get_string("msg")) }
    else if et == "stall" { ("stall".to_string(), String::new()) }
    else { ("fail".to_string(), String::new()) };
  let out = if r.outcome == "fail" { String::new() } else { canon_obj(&r.out) };
  // A fail rule ends in the "done" event and the budget in an event of run's own: neither has a place.
  let place = if (et == "error" || et == "stall") && code != "budget" {
    let frame = e.get_array("frame");
    let path: Vec<String> = frame.objects().iter().map(|p| { let p = p.array(); format!("[{},{}]", p.get_int(0), canon(&p.get_property(1))) }).collect();
    format!("{}|case {}", path.join(""), e.get_int("case"))
  } else { String::new() };
  let ints = |k: &str| -> Vec<usize> { e.get_array(k).objects().iter().map(|d| d.int() as usize).collect() };
  let stall = if et == "stall" { format!("{:?} {:?} {:?}", ints("wires"), ints("syncs"), ints("ops")) } else { String::new() };
  Seen { outcome: r.outcome, code, msg, place, stall, steps: r.steps, out, globals: snapshot_globals(), args: canon_obj(&a) }
}

fn via_fastrun(case: &Case, args: &DataObject, budget: i64) -> Seen {
  preset_globals();
  let a = args.deep_copy();
  let r = fastrun::run_report(case, a.clone(), budget);
  let out = if r.outcome == "fail" { String::new() } else { canon_obj(&r.out) };
  // The stepper has no message for a fail rule or a stall; the fast path describes them.
  let msg = if r.code == "fail" || r.code == "stall" { String::new() } else { r.msg.clone() };
  let place = match &r.error {
    Some(err) if r.code != "fail" && r.code != "budget" => {
      let n = err.at.len();
      if n == 0 { "<no site>".to_string() } else {
        let path: Vec<String> = err.at[..n - 1].iter().map(|s| format!("[{},{}]", s.op.map(|o| o as i64).unwrap_or(-1), s.iter.map(|i| i.to_string()).unwrap_or("null".to_string()))).collect();
        format!("{}|case {}", path.join(""), err.at[n - 1].case)
      }
    }
    _ => String::new(),
  };
  let stall = match &r.stall { Some((w, sy, o)) if r.code == "stall" => format!("{:?} {:?} {:?}", w, sy, o), _ => String::new() };
  Seen { outcome: r.outcome, code: r.code, msg, place, stall, steps: r.steps, out, globals: snapshot_globals(), args: canon_obj(&a) }
}

/// ndata error texts name heap slots ("at ref 19390", "DArray(4048)"), which differ between
/// any two runs; replace those numbers.
fn norm(s: &str) -> String {
  let marks = ["ref ", "DArray(", "DObject(", "DBytes("];
  let mut out = String::with_capacity(s.len());
  let mut rest = s;
  loop {
    let next = marks.iter().filter_map(|m| rest.find(m).map(|p| (p, m.len()))).min();
    let (p, l) = match next { Some(x) => x, None => break };
    out.push_str(&rest[..p + l]);
    rest = &rest[p + l..];
    let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 { out.push('N'); rest = &rest[digits..]; }
  }
  out.push_str(rest);
  out
}

/// Where several of an op's terminals are at fault, the stepper's message names the first in the
/// op's key order, which is random in both runs; compare those messages without the name.
fn norm_msg(s: &str) -> String {
  let mut m = norm(s);
  for lead in ["lacks output '", "list input '", "loop output '", "command op output '"] {
    if let Some(p) = m.find(lead) {
      let start = p + lead.len();
      if let Some(q) = m[start..].find('\'') { m.replace_range(start..start + q, "*"); }
    }
  }
  m
}

/// Compare one flow under both; returns a description of the first difference.
fn compare(case: &Case, args: &DataObject, budget: i64) -> Result<Seen, String> {
  let trace = std::env::var("FASTRUN_PARITY_TRACE").is_ok();
  if trace { eprintln!("  stepper (budget {}) args {}", budget, canon_obj(args)); }
  let s = via_stepper(case, args, budget);
  if trace { eprintln!("  fastrun"); }
  let f = via_fastrun(case, args, budget);
  let fields = [
    ("outcome", &s.outcome, &f.outcome), ("code", &s.code, &f.code), ("msg", &s.msg, &f.msg),
    ("place", &s.place, &f.place), ("stall", &s.stall, &f.stall),
    ("out", &s.out, &f.out), ("globals", &s.globals, &f.globals), ("args", &s.args, &f.args),
  ];
  for (name, a, b) in fields {
    let same = if name == "msg" { norm_msg(a) == norm_msg(b) } else { norm(a) == norm(b) };
    if !same { return Err(format!("{} differs:\n  stepper: {}\n  fastrun: {}", name, a, b)); }
  }
  if s.steps != f.steps { return Err(format!("steps differ: stepper {} fastrun {}", s.steps, f.steps)); }
  Ok(s)
}

/// Fail the test loudly: the quiet panic hook set for the runs would hide the message.
fn fail(msg: String) -> ! {
  let _ = std::panic::take_hook();
  panic!("{}", msg);
}

// ---- a tiny deterministic RNG ----

struct Rng(u64);
impl Rng {
  fn next(&mut self) -> u64 { let mut x = self.0; x ^= x << 13; x ^= x >> 7; x ^= x << 17; self.0 = x; x }
  fn below(&mut self, n: usize) -> usize { (self.next() % n as u64) as usize }
  fn chance(&mut self, pct: usize) -> bool { self.below(100) < pct }
  fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T { &v[self.below(v.len())] }
}

// ---- the flow generator ----

const NAMES: [&str; 4] = ["a", "b", "c", "x"];
const OUT_NAMES: [&str; 3] = ["r", "s", "a"];

// pure primitives whose results do not depend on an object's key order (no `push`: push(x, x)
// makes an array contain itself, and the stepper's deep-copied events overflow the stack on it)
const PRIMS: [(&str, &[&str]); 20] = [
  ("+", &["a", "b"]), ("-", &["a", "b"]), ("*", &["a", "b"]), ("/", &["a", "b"]),
  ("<", &["a", "b"]), (">", &["a", "b"]), ("or", &["a", "b"]), ("equals", &["a", "b"]),
  ("length", &["a"]), ("split", &["a", "b"]), ("trim", &["a"]), ("get", &["a", "b"]),
  ("get_or_null", &["a", "b"]), ("has", &["a", "b"]), ("remove", &["a", "b"]),
  ("index_of", &["a", "b"]), ("starts_with", &["a", "b"]), ("is_string", &["a"]), ("to_int", &["a"]),
  ("no_such_primitive", &["a"]),
];

// command op callees in data/testflow (one output each), and broken references
const CALLEES: [(&str, &[&str]); 6] = [
  ("testflow:testflow:zkuwhn1802d57cb8ak1c", &["a", "b"]), // test_add (flow)
  ("testflow:testflow:ooizjt1803765b08ak212", &["a"]),     // test_conditionals (flow, case chain)
  ("testflow:testflow:nrnlzl1807a95053fz96c", &["a"]),     // test_rust (rust)
  ("testflow:testflow:no_such_command", &["a"]),           // fails to load
  ("testflow:only_two", &["a"]),                           // not lib:ctl:cmd
  ("", &["a"]),                                            // no cmd at all (key omitted)
];

fn node(mode: &str) -> DataObject {
  let mut n = DataObject::new();
  n.put_string("mode", mode); n.put_string("type", "Any"); n.put_float("x", 0.0);
  n
}
fn loop_node(target: Option<&str>) -> DataObject {
  let mut n = node("loop");
  if let Some(t) = target { n.put_string("loop", t); }
  n
}
fn bar(names: &[String]) -> DataObject {
  let mut o = DataObject::new();
  for k in names { o.put_object(k, node("regular")); }
  o
}
fn subset(rng: &mut Rng, pool: &[&str], min: usize, max: usize) -> Vec<String> {
  let n = min + rng.below(max - min + 1);
  let mut v: Vec<String> = Vec::new();
  while v.len() < n.min(pool.len()) { let k = rng.pick(pool).to_string(); if !v.contains(&k) { v.push(k); } }
  v
}

fn literal(rng: &mut Rng, ctype: &str) -> String {
  match ctype {
    "int" => rng.pick(&["0", "1", "3", "-2", "42", "x1"]).to_string(),
    "decimal" => rng.pick(&["0.5", "2.0", "-1.25", "nope"]).to_string(),
    "boolean" => rng.pick(&["true", "false", "maybe"]).to_string(),
    "string" => rng.pick(&["", "hi", "1,2,3", "a", "1", "say \\\"hi\\\"", "tab\\tx", "bad\"quote", "back\\"]).to_string(),
    "object" => rng.pick(&["{\"k\":1}", "{}", "{\"k\":[1,2]}", "{broken"]).to_string(),
    "array" => rng.pick(&["[1,2,3]", "[]", "[\"a\",\"b\"]", "[[1],[2]]", "[oops"]).to_string(),
    "null" => "null".to_string(),
    _ => "whatever".to_string(),
  }
}

struct OpShape { ins: Vec<String>, outs: Vec<String> }

fn gen_case(rng: &mut Rng, depth: usize, inputs: &[String], outputs: &[String]) -> DataObject {
  let nops = 1 + rng.below(6);
  let mut cmds = DataArray::new();
  let mut shapes: Vec<OpShape> = Vec::new();
  for _ in 0..nops {
    let mut op = DataObject::new();
    let mut pos = DataObject::new(); pos.put_float("x", 0.0); pos.put_float("y", 0.0); pos.put_float("z", 0.0);
    op.put_object("pos", pos); op.put_float("width", 1.5);
    let kind = rng.below(100);
    let (typ, ins, mut outs): (&str, Vec<String>, Vec<String>);
    if kind < 33 {
      let (name, ar) = *rng.pick(&PRIMS);
      op.put_string("name", name);
      typ = "primitive";
      ins = ar.iter().map(|s| s.to_string()).collect();
      outs = vec![if rng.chance(90) { "a".to_string() } else { "q".to_string() }];
    } else if kind < 50 {
      typ = "constant";
      let ct = *rng.pick(&["int", "decimal", "boolean", "string", "object", "array", "weird"]);
      if !rng.chance(3) { op.put_string("ctype", ct); }
      op.put_string("name", &literal(rng, ct));
      ins = vec![];
      outs = subset(rng, &NAMES, 1, 2);
    } else if kind < 60 {
      typ = "persistent";
      op.put_string("name", rng.pick(&GLOBALS));
      ins = if rng.chance(50) { vec![rng.pick(&NAMES).to_string()] } else { vec![] };
      outs = subset(rng, &NAMES, 0, 2);
    } else if kind < 70 {
      typ = "match";
      let ct = *rng.pick(&["int", "string", "boolean", "null", "decimal", "array"]);
      if !rng.chance(3) { op.put_string("ctype", ct); }
      op.put_string("name", &literal(rng, ct));
      ins = if rng.chance(95) { vec!["a".to_string()] } else { vec![] };
      outs = if rng.chance(85) { vec![] } else { vec!["a".to_string()] };
    } else if kind < 82 {
      typ = "local";
      op.put_string("name", "local");
      ins = subset(rng, &NAMES, 0, 2);
      outs = subset(rng, &NAMES, 1, 2);
      if depth < 3 && rng.chance(90) { op.put_object("localdata", gen_case(rng, depth + 1, &ins, &outs)); }
    } else if kind < 92 {
      typ = "command";
      let (cmd, ar) = *rng.pick(&CALLEES);
      if !cmd.is_empty() { op.put_string("cmd", cmd); }
      op.put_string("name", "call");
      ins = ar.iter().map(|s| s.to_string()).collect();
      outs = vec![rng.pick(&["result", "a"]).to_string()];
    } else if kind < 96 {
      typ = "undefined";
      op.put_string("name", "undefined");
      ins = subset(rng, &NAMES, 0, 1);
      outs = subset(rng, &NAMES, 0, 1);
    } else {
      typ = "no_such_type";
      op.put_string("name", "mystery");
      ins = vec![];
      outs = subset(rng, &NAMES, 0, 1);
    }
    op.put_string("type", typ);
    // modes: list inputs; list and loop outputs (a loop never feeds a list input)
    let single_in = typ == "persistent" || typ == "match";
    let mut list_ins: Vec<String> = Vec::new();
    let mut in_o = DataObject::new();
    for k in &ins {
      let m = if !single_in && rng.chance(12) { list_ins.push(k.clone()); "list" } else { "regular" };
      in_o.put_object(k, node(m));
    }
    let mut out_o = DataObject::new();
    if typ == "command" { outs.truncate(1); }
    for k in &outs {
      let r = rng.below(100);
      if r < 12 { out_o.put_object(k, node("list")); }
      else if r < 18 {
        let plain: Vec<&String> = ins.iter().filter(|x| !list_ins.contains(x)).collect();
        let target = if rng.chance(10) { None } else if !plain.is_empty() && rng.chance(80) { Some(rng.pick(&plain).as_str()) } else { Some("zz") };
        out_o.put_object(k, loop_node(target));
      } else { out_o.put_object(k, node("regular")); }
    }
    op.put_object("in", in_o);
    op.put_object("out", out_o);
    if rng.chance(if typ == "match" { 70 } else { 22 }) {
      let mut c = DataObject::new();
      c.put_string("rule", rng.pick(&["next", "terminate", "fail", "finish", "finish", "bogus"]));
      c.put_boolean("value", rng.chance(50));
      op.put_object("condition", c);
    }
    cmds.push_object(op);
    shapes.push(OpShape { ins, outs });
  }
  // wires
  let n = shapes.len() as i64;
  let mut cons: Vec<DataObject> = Vec::new();
  let src_for = |rng: &mut Rng, dest_op: Option<usize>| -> (i64, String) {
    let r = rng.below(100);
    if r < 3 { return (*rng.pick(&[-5i64, 99]), "a".to_string()); }
    let limit = match dest_op { Some(d) if r >= 8 => d, _ => shapes.len() };
    let cands: Vec<usize> = (0..limit).filter(|&j| !shapes[j].outs.is_empty()).collect();
    if cands.is_empty() || rng.chance(30) {
      let nm = if !inputs.is_empty() && rng.chance(90) { rng.pick(inputs).clone() } else { "nope".to_string() };
      return (-1, nm);
    }
    let j = *rng.pick(&cands);
    let nm = if rng.chance(95) { rng.pick(&shapes[j].outs).clone() } else { "ghost".to_string() };
    (j as i64, nm)
  };
  let mk = |s: (i64, String), d: (i64, String)| -> DataObject {
    let mut c = DataObject::new();
    let mut sa = DataArray::new(); sa.push_int(s.0); sa.push_string(&s.1);
    let mut da = DataArray::new(); da.push_int(d.0); da.push_string(&d.1);
    c.put_array("src", sa); c.put_array("dest", da);
    c
  };
  for (i, sh) in shapes.iter().enumerate() {
    for k in &sh.ins {
      if rng.chance(85) { let s = src_for(rng, Some(i)); cons.push(mk(s, (i as i64, k.clone()))); }
      if rng.chance(7) { let s = src_for(rng, Some(i)); cons.push(mk(s, (i as i64, k.clone()))); }
    }
  }
  for k in outputs { if rng.chance(90) { let s = src_for(rng, None); cons.push(mk(s, (-2, k.clone()))); } }
  if rng.chance(10) { let s = src_for(rng, None); cons.push(mk(s, (-2, "extra".to_string()))); }
  if rng.chance(3) { let s = src_for(rng, None); cons.push(mk(s, (*rng.pick(&[-7i64, 50]), "a".to_string()))); }
  if rng.chance(4) && n > 0 { let s = src_for(rng, None); cons.push(mk(s, (rng.below(n as usize) as i64, "not_an_input".to_string()))); }
  // shuffle the wires: their order is the sweep order
  for i in (1..cons.len()).rev() { let j = rng.below(i + 1); cons.swap(i, j); }
  let mut cons_a = DataArray::new();
  for c in cons { cons_a.push_object(c); }
  let mut case = DataObject::new();
  case.put_object("input", bar(inputs));
  case.put_object("output", bar(outputs));
  case.put_array("cmds", cmds);
  case.put_array("cons", cons_a);
  if rng.chance(30) {
    let mut syncs = DataArray::new();
    for _ in 0..(1 + rng.below(3)) {
      let mut s = DataObject::new();
      let pick = |rng: &mut Rng| if rng.chance(8) { -1 } else { rng.below(n as usize + 1) as i64 };
      s.put_int("src", pick(rng)); s.put_int("dest", pick(rng));
      syncs.push_object(s);
    }
    case.put_array("syncs", syncs);
  }
  if depth < 3 && rng.chance(25) { case.put_object("nextcase", gen_case(rng, depth + 1, inputs, outputs)); }
  case
}

fn gen_value(rng: &mut Rng) -> Data {
  match rng.below(9) {
    0 => Data::DInt(rng.below(5) as i64),
    1 => Data::DInt(-3),
    2 => Data::DString(rng.pick(&["1,2,3", "hi", "", "3"]).to_string()),
    3 => Data::DArray(DataArray::from_string(rng.pick(&["[1,2,3]", "[]", "[\"a\",\"b\"]", "[4]"])).data_ref),
    4 => Data::DObject(DataObject::from_string(rng.pick(&["{\"k\":1}", "{}"])).data_ref),
    5 => Data::DBoolean(rng.chance(50)),
    6 => Data::DFloat(1.5),
    7 => Data::DNull,
    _ => Data::DString("true".to_string()),
  }
}

fn gen_args(rng: &mut Rng) -> DataObject {
  let mut a = DataObject::new();
  for k in NAMES { if rng.chance(85) { let v = gen_value(rng); a.set_property(k, v); } }
  a
}

/// `levels` locals nested inside each other around a constant: the run opens levels + 1 frames.
fn nested_locals(levels: usize) -> DataObject {
  let mut pos = DataObject::new(); pos.put_float("x", 0.0); pos.put_float("y", 0.0); pos.put_float("z", 0.0);
  let wire_out = || {
    let mut c = DataObject::new();
    let mut sa = DataArray::new(); sa.push_int(0); sa.push_string("r");
    let mut da = DataArray::new(); da.push_int(-2); da.push_string("r");
    c.put_array("src", sa); c.put_array("dest", da);
    let mut a = DataArray::new(); a.push_object(c); a
  };
  let out_bar = || { let mut o = DataObject::new(); o.put_object("r", node("regular")); o };
  let mut op = DataObject::new();
  op.put_string("type", "constant"); op.put_string("ctype", "string"); op.put_string("name", "deep");
  op.put_object("in", DataObject::new()); op.put_object("out", out_bar());
  op.put_object("pos", pos.deep_copy()); op.put_float("width", 1.0);
  for _ in 0..=levels {
    let mut case = DataObject::new();
    case.put_object("input", DataObject::new()); case.put_object("output", out_bar());
    let mut cmds = DataArray::new(); cmds.push_object(op); case.put_array("cmds", cmds);
    case.put_array("cons", wire_out());
    let mut local = DataObject::new();
    local.put_string("type", "local"); local.put_string("name", "nest");
    local.put_object("in", DataObject::new()); local.put_object("out", out_bar());
    local.put_object("pos", pos.deep_copy()); local.put_float("width", 1.0);
    local.put_object("localdata", case);
    op = local;
  }
  op.get_object("localdata")
}

#[test]
fn fastrun_matches_the_stepper() {
  flowlang::init("data");
  // Panics inside primitives and commands are caught by both runs; keep their noise out.
  std::panic::set_hook(Box::new(|_| {}));

  // 1. The repository's own flows (test_speed reads the clock, so only its shape is compared).
  let store = DataStore::new();
  let fixtures: [(&str, &str, &[&str]); 5] = [
    ("test_add", "gukzkk1802d57d9f0w1d", &["{\"a\":300,\"b\":120}", "{\"a\":\"x\",\"b\":1}", "{}"]),
    ("test_lists", "nzgmnn1803778b9d9t3a6", &["{\"a\":[1,2,3]}", "{\"a\":[]}", "{\"a\":\"x\"}", "{\"a\":[1,\"q\",{}]}"]),
    ("test_loop", "mmtmoy18037d7a5f8t468", &["{\"a\":0}", "{\"a\":2}"]),
    ("test_conditionals", "mkqrzt1803765bdc5q213", &["{\"a\":true}", "{\"a\":false}", "{}"]),
    ("test_command", "jgtjzi1802d67c66cq20", &["{\"a\":5}", "{\"a\":\"z\"}"]),
  ];
  let mut fixture_runs = 0;
  for (name, implid, argsets) in fixtures.iter() {
    let body = store.get_data("testflow", implid).get_object("data").get_object("flow");
    let case = Case::from_data(body);
    for a in argsets.iter() {
      if let Err(e) = compare(&case, &DataObject::from_string(a), 0) { fail(format!("fixture {} {}: {}", name, a, e)); }
      fixture_runs += 1;
    }
  }
  // test_speed: a countdown loop that reads the clock. Same steps and outcome; the values differ.
  {
    let body = store.get_data("testflow", "uqoztz18041a7027eg318").get_object("data").get_object("flow");
    let case = Case::from_data(body);
    let s = via_stepper(&case, &DataObject::from_string("{\"a\":50}"), 0);
    let f = via_fastrun(&case, &DataObject::from_string("{\"a\":50}"), 0);
    assert_eq!((s.outcome, s.steps), (f.outcome, f.steps), "test_speed");
  }

  // The frame depth limit (64, root included), at and around it.
  for levels in [1usize, 62, 63, 64, 70] {
    let case = Case::from_data(nested_locals(levels));
    match compare(&case, &DataObject::new(), 0) {
      Ok(s) => {
        let want = if levels + 1 <= 64 { "ok" } else { "depth" };
        let got = if s.outcome == "fail" { s.code.as_str() } else { s.outcome.as_str() };
        if got != want { fail(format!("{} nested locals: {} (wanted {})", levels, got, want)); }
      }
      Err(e) => fail(format!("{} nested locals: {}", levels, e)),
    }
  }

  // 2. Random flows, each with a few argument sets and budgets.
  let seeds: u64 = std::env::var("FASTRUN_PARITY_SEEDS").ok().and_then(|s| s.parse().ok()).unwrap_or(3000);
  let mut tally: std::collections::BTreeMap<String, usize> = Default::default();
  let first: u64 = std::env::var("FASTRUN_PARITY_FIRST").ok().and_then(|s| s.parse().ok()).unwrap_or(1);
  for seed in first..=seeds {
    let mut rng = Rng(0x9E3779B97F4A7C15 ^ seed.wrapping_mul(0x2545F4914F6CDD1D));
    let inputs = subset(&mut rng, &NAMES, 1, 3);
    let outputs = subset(&mut rng, &OUT_NAMES, 1, 2);
    let body = gen_case(&mut rng, 0, &inputs, &outputs);
    if std::env::var("FASTRUN_PARITY_TRACE").is_ok() { eprintln!("seed {}\n{}", seed, body.to_string()); }
    let case = Case::from_data(body.clone());
    // An open loop can grow its data without bound (an error envelope fed back into the
    // input that produced it nests its own text, about doubling each pass), so a body with a
    // loop output runs under small budgets; list iterations are bounded by their inputs.
    let loops = body.to_string().contains("\"mode\":\"loop\"");
    for round in 0..3 {
      let args = gen_args(&mut rng);
      let budget = match (round, loops) {
        (0, false) => 3000, (0, true) => 48,
        (1, _) => 1 + rng.below(48) as i64,
        (_, false) => 400, (_, true) => 30,
      };
      match compare(&case, &args, budget) {
        Ok(s) => {
          if s.place.starts_with('[') { *tally.entry("(in a nested frame)".to_string()).or_default() += 1; }
          *tally.entry(if s.outcome == "fail" { s.code } else { s.outcome }).or_default() += 1;
        }
        Err(e) => fail(format!("seed {} round {} budget {}: {}\nargs: {}\nbody: {}", seed, round, budget, e, canon_obj(&args), body.to_string())),
      }
    }
  }
  eprintln!("fastrun parity: {} fixture runs, {} random flows x 3 runs; outcomes {:?}", fixture_runs, seeds, tally);
  let _ = std::panic::take_hook();
  // The generator must actually reach the interesting endings, or the check proves little.
  for k in ["ok", "terminate", "fail", "stall", "budget", "I-4", "absent-output", "panic", "(in a nested frame)"] {
    assert!(tally.get(k).copied().unwrap_or(0) > 0, "no random flow ended with '{}': {:?}", k, tally);
  }

  // 3. Code::execute is the fast path, with the stepper's outcome mapping.
  let add = store.get_data("testflow", "gukzkk1802d57d9f0w1d").get_object("data").get_object("flow");
  let mut code = Code::new(Case::from_data(add));
  let r = code.execute(DataObject::from_string("{\"a\":300,\"b\":120}")).expect("test_add runs");
  assert_eq!(r.get_string("a"), "300+120=420");
}
