//! Synchros: data-less op -> op ordering edges (`Case.syncs`).
//!
//! A synchro `{src, dest}` says "op src must finish before op dest starts"
//! and carries no value. Code::execute honors it: an op with an incoming
//! synchro does not run at case start; when its inputs land it also waits
//! for every incoming synchro's src; the sweep fires a synchro once its src
//! is done and runs a now-ready dest exactly once; a case ends only when
//! every wire AND every synchro has fired.
//!
//! The fixtures are the newbound "Synchros S0" flows verbatim. Each one
//! gives a different result when synchros are ignored, so a runtime that
//! drops `syncs` fails this suite. Persistent ops make ordering visible:
//! each fixture presets its own global to a "previous" value first.
//!
//! One #[test] fn on purpose: the DataStore/ndata globals initialize once
//! per process, so the phases run as one sequence in one process.
//!
//! The stalling fixture (a wire + synchro cycle) is deliberately absent:
//! a stuck case loops forever under Code::execute today.

use flowlang::case::Case;
use flowlang::code::Code;
use flowlang::datastore::DataStore;
use ndata::dataobject::DataObject;

const FIXTURE_A: &str = r#"{"cmds":[{"in":{"a":{"mode":"in","type":"Any","x":0}},"name":"synchro_s0_fixture_a","out":{"stored":{"mode":"out","type":"Any","x":0}},"pos":{"x":-1,"y":0.5,"z":0},"type":"persistent","width":2.5},{"in":{},"name":"synchro_s0_fixture_a","out":{"b":{"mode":"out","type":"Any","x":0}},"pos":{"x":1,"y":-0.5,"z":0},"type":"persistent","width":2.5}],"cons":[{"dest":[0,"a"],"src":[-1,"v"]},{"dest":[-2,"r"],"src":[1,"b"]}],"input":{"v":{"mode":"in","type":"String","x":0}},"output":{"r":{"mode":"out","type":"String","x":0}},"syncs":[{"dest":1,"src":0}]}"#;
const FIXTURE_A_NOSYNC: &str = r#"{"cmds":[{"in":{"a":{"mode":"in","type":"Any","x":0}},"name":"synchro_s0_fixture_a","out":{"stored":{"mode":"out","type":"Any","x":0}},"pos":{"x":-1,"y":0.5,"z":0},"type":"persistent","width":2.5},{"in":{},"name":"synchro_s0_fixture_a","out":{"b":{"mode":"out","type":"Any","x":0}},"pos":{"x":1,"y":-0.5,"z":0},"type":"persistent","width":2.5}],"cons":[{"dest":[0,"a"],"src":[-1,"v"]},{"dest":[-2,"r"],"src":[1,"b"]}],"input":{"v":{"mode":"in","type":"String","x":0}},"output":{"r":{"mode":"out","type":"String","x":0}}}"#;
const FIXTURE_B: &str = r#"{"cmds":[{"condition":{"rule":"next","value":false},"ctype":"string","in":{"a":{"mode":"in","type":"Any","x":0}},"name":"never","out":{},"pos":{"x":-1,"y":0.6,"z":0},"type":"match","width":1.5},{"in":{"m":{"mode":"in","type":"Any","x":0}},"name":"synchro_s0_fixture_b","out":{},"pos":{"x":1,"y":-0.5,"z":0},"type":"persistent","width":2.5},{"ctype":"string","in":{},"name":"dest_ran","out":{"c":{"mode":"out","type":"String","x":0}},"pos":{"x":1,"y":0.6,"z":0},"type":"constant","width":1.5}],"cons":[{"dest":[1,"m"],"src":[2,"c"]},{"dest":[0,"a"],"src":[-1,"v"]}],"input":{"v":{"mode":"in","type":"String","x":0}},"nextcase":{"cmds":[{"ctype":"string","in":{},"name":"case2","out":{"c":{"mode":"out","type":"String","x":0}},"pos":{"x":-1,"y":0,"z":0},"type":"constant","width":1.5},{"in":{},"name":"synchro_s0_fixture_b","out":{"m":{"mode":"out","type":"Any","x":0}},"pos":{"x":1,"y":0,"z":0},"type":"persistent","width":2.5}],"cons":[{"dest":[-2,"r"],"src":[0,"c"]},{"dest":[-2,"marker"],"src":[1,"m"]}],"input":{"v":{"mode":"in","type":"String","x":0}},"output":{"marker":{"mode":"out","type":"Any","x":0},"r":{"mode":"out","type":"String","x":0}}},"output":{"marker":{"mode":"out","type":"Any","x":0},"r":{"mode":"out","type":"String","x":0}},"syncs":[{"dest":1,"src":0}]}"#;
const FIXTURE_C: &str = r#"{"cmds":[{"in":{"a":{"mode":"in","type":"Any","x":0},"b":{"mode":"in","type":"Any","x":0}},"name":"split","out":{"a":{"mode":"out","type":"Any","x":0}},"pos":{"x":-1.5,"y":1,"z":0},"type":"primitive","width":1.5},{"in":{"a":{"mode":"list","type":"Any","x":0}},"name":"synchro_s0_fixture_c","out":{"each":{"mode":"list","type":"Any","x":0}},"pos":{"x":-1,"y":0,"z":0},"type":"persistent","width":2.5},{"in":{},"name":"synchro_s0_fixture_c","out":{"b":{"mode":"out","type":"Any","x":0}},"pos":{"x":1,"y":-1,"z":0},"type":"persistent","width":2.5},{"ctype":"string","in":{},"name":",","out":{"c":{"mode":"out","type":"String","x":0}},"pos":{"x":-0.5,"y":2,"z":0},"type":"constant","width":1}],"cons":[{"dest":[0,"a"],"src":[-1,"items"]},{"dest":[0,"b"],"src":[3,"c"]},{"dest":[1,"a"],"src":[0,"a"]},{"dest":[-2,"seen"],"src":[2,"b"]},{"dest":[-2,"iterations"],"src":[1,"each"]}],"input":{"items":{"mode":"in","type":"String","x":0}},"output":{"iterations":{"mode":"out","type":"JSONArray","x":0},"seen":{"mode":"out","type":"Any","x":0}},"syncs":[{"dest":2,"src":1}]}"#;
const FIXTURE_D: &str = r#"{"cmds":[{"in":{"a":{"mode":"in","type":"Any","x":0}},"name":"synchro_s0_fixture_d","out":{},"pos":{"x":-1.5,"y":0.5,"z":0},"type":"persistent","width":2.5},{"in":{},"localdata":{"cmds":[{"in":{},"name":"synchro_s0_fixture_d","out":{"a":{"mode":"out","type":"Any","x":0}},"pos":{"x":0,"y":0.5,"z":0},"type":"persistent","width":2.5},{"in":{"a":{"mode":"in","type":"Any","x":0}},"name":"synchro_s0_fixture_d2","out":{"a":{"mode":"out","type":"Any","x":0}},"pos":{"x":0,"y":-0.5,"z":0},"type":"persistent","width":2.5}],"cons":[{"dest":[1,"a"],"src":[0,"a"]},{"dest":[-2,"x"],"src":[1,"a"]}],"input":{},"output":{"x":{"mode":"out","type":"Any","x":0}}},"name":"copy_d_to_d2","out":{"x":{"mode":"out","type":"Any","x":0}},"pos":{"x":0,"y":0,"z":0},"type":"local","width":2},{"in":{},"name":"synchro_s0_fixture_d2","out":{"b":{"mode":"out","type":"Any","x":0}},"pos":{"x":1.5,"y":-0.5,"z":0},"type":"persistent","width":2.5}],"cons":[{"dest":[0,"a"],"src":[-1,"v"]},{"dest":[-2,"l"],"src":[1,"x"]},{"dest":[-2,"r"],"src":[2,"b"]}],"input":{"v":{"mode":"in","type":"String","x":0}},"output":{"l":{"mode":"out","type":"Any","x":0},"r":{"mode":"out","type":"Any","x":0}},"syncs":[{"dest":1,"src":0},{"dest":2,"src":1}]}"#;

// A wire lands on the dest BEFORE the synchro's src is done: con 0 feeds
// op 1 (a local that reads the global) ahead of con 1, which feeds op 0
// (the writer). Unsynchronized, op 1 runs on landing and reads "previous".
const FIXTURE_WIRE_FIRST: &str = r#"{"input":{"v":{"mode":"in","type":"String","x":0}},"output":{"r":{"mode":"out","type":"Any","x":0}},"cmds":[{"in":{"a":{"mode":"in","type":"Any","x":0}},"out":{},"name":"synchro_test_wire_first","pos":{"x":0,"y":0,"z":0},"type":"persistent","width":2},{"in":{"a":{"mode":"in","type":"Any","x":0}},"out":{"x":{"mode":"out","type":"Any","x":0}},"name":"reader","pos":{"x":0,"y":0,"z":0},"type":"local","width":2,"localdata":{"input":{"a":{"mode":"in","type":"Any","x":0}},"output":{"x":{"mode":"out","type":"Any","x":0}},"cmds":[{"in":{},"out":{"b":{"mode":"out","type":"Any","x":0}},"name":"synchro_test_wire_first","pos":{"x":0,"y":0,"z":0},"type":"persistent","width":2}],"cons":[{"src":[0,"b"],"dest":[-2,"x"]}]}}],"cons":[{"src":[-1,"v"],"dest":[1,"a"]},{"src":[-1,"v"],"dest":[0,"a"]},{"src":[1,"x"],"dest":[-2,"r"]}],"syncs":[{"src":0,"dest":1}]}"#;

fn run(body: &str, args: &str) -> DataObject {
  let mut code = Code::new(Case::new(body));
  code.execute(DataObject::from_string(args)).expect("flow failed")
}

fn preset(name: &str, value: &str) {
  DataStore::globals().put_string(name, value);
}

fn without_syncs(body: &str) -> String {
  let mut o = DataObject::from_string(body);
  o.remove_property("syncs");
  o.to_string()
}

#[test]
fn synchros() {
  let dir = std::env::temp_dir().join("flowlang_synchros_test");
  let _ = std::fs::create_dir_all(&dir);
  DataStore::init(dir.to_str().unwrap());

  // --- parsing: absent = empty, present = kept, duplicate copies them ---
  let a = Case::new(FIXTURE_A);
  assert_eq!(a.syncs.len(), 1);
  assert_eq!((a.syncs[0].src, a.syncs[0].dest, a.syncs[0].done), (0, 1, false));
  let dup = a.duplicate();
  assert_eq!((dup.syncs.len(), dup.syncs[0].src, dup.syncs[0].dest), (1, 0, 1));
  assert_eq!(Case::new(FIXTURE_A_NOSYNC).syncs.len(), 0);
  let d = Case::new(FIXTURE_D);
  assert_eq!(d.syncs.len(), 2);
  assert_eq!(d.cmds[1].localdata.as_ref().unwrap().syncs.len(), 0);
  let b = Case::new(FIXTURE_B);
  assert_eq!(b.nextcase.as_ref().unwrap().syncs.len(), 0);

  // --- (a) A stores params.v, B (no input) reads; synchro A->B ---
  preset("synchro_s0_fixture_a", "previous");
  let r = run(FIXTURE_A, r#"{"v":"fresh"}"#);
  assert_eq!(r.get_string("r"), "fresh", "(a) B must wait for A");
  // control: the same body without syncs - B runs at case start
  preset("synchro_s0_fixture_a", "previous");
  let r = run(FIXTURE_A_NOSYNC, r#"{"v":"fresh"}"#);
  assert_eq!(r.get_string("r"), "previous", "(a) control is unsynchronized");

  // --- (b) src's next rule fires first: dest never runs, case 2 answers ---
  preset("synchro_s0_fixture_b", "untouched");
  let r = run(FIXTURE_B, r#"{"v":"go"}"#);
  assert_eq!(r.get_string("r"), "case2");
  assert_eq!(r.get_string("marker"), "untouched", "(b) dest must not run after an early next");
  preset("synchro_s0_fixture_b", "untouched");
  let r = run(&without_syncs(FIXTURE_B), r#"{"v":"go"}"#);
  assert_eq!(r.get_string("marker"), "dest_ran", "(b) control: unsynchronized dest runs");

  // --- (c) list-mode src: dest sees every iteration ---
  preset("synchro_s0_fixture_c", "previous");
  let r = run(FIXTURE_C, r#"{"items":"x1,x2,x3"}"#);
  assert_eq!(r.get_string("seen"), "x3", "(c) dest must see the last iteration");
  assert_eq!(r.get_array("iterations").to_string(), r#"["x1","x2","x3"]"#);
  preset("synchro_s0_fixture_c", "previous");
  let r = run(&without_syncs(FIXTURE_C), r#"{"items":"x1,x2,x3"}"#);
  assert_eq!(r.get_string("seen"), "previous", "(c) control");

  // --- (d) a synchro into and out of a local op ---
  preset("synchro_s0_fixture_d", "previous_d");
  preset("synchro_s0_fixture_d2", "previous_d2");
  let r = run(FIXTURE_D, r#"{"v":"fresh"}"#);
  assert_eq!(r.get_string("l"), "fresh", "(d) the local must start after W");
  assert_eq!(r.get_string("r"), "fresh", "(d) R must wait for the whole local");
  preset("synchro_s0_fixture_d", "previous_d");
  preset("synchro_s0_fixture_d2", "previous_d2");
  let r = run(&without_syncs(FIXTURE_D), r#"{"v":"fresh"}"#);
  assert_eq!(r.get_string("r"), "previous_d", "(d) control");

  // --- a wire lands on dest before src is done: dest still waits ---
  preset("synchro_test_wire_first", "previous");
  let r = run(FIXTURE_WIRE_FIRST, r#"{"v":"fresh"}"#);
  assert_eq!(r.get_string("r"), "fresh", "a landed wire must not bypass the synchro");
  preset("synchro_test_wire_first", "previous");
  let r = run(&without_syncs(FIXTURE_WIRE_FIRST), r#"{"v":"fresh"}"#);
  assert_eq!(r.get_string("r"), "previous", "wire-first control");

  // --- malformed endpoints never gate (and never hang the case) ---
  for bad in [r#"[{"src":1,"dest":1}]"#, r#"[{"src":0,"dest":9}]"#, r#"[{"src":-1,"dest":1}]"#] {
    let mut o = DataObject::from_string(FIXTURE_A);
    o.put_array("syncs", ndata::dataarray::DataArray::from_string(bad));
    preset("synchro_s0_fixture_a", "previous");
    let r = run(&o.to_string(), r#"{"v":"fresh"}"#);
    assert_eq!(r.get_string("r"), "previous", "malformed synchro {} must be inert", bad);
  }
}
