//! End-to-end proof of the hotswap loader against real dylibs that speak
//! the full mirror handshake: store-declared discovery, command execution
//! through the shared ndata heap, deterministic reload with changed
//! behavior, the stale-id sweep, never-unload (a generation-1 transform
//! pointer stays callable after generation 2 loads), and the ABI contract
//! refusing a mismatched library.
//!
//! The fixture dylibs are compiled by the test itself with plain rustc,
//! linking the exact ndata rlib this test binary was built against (found
//! in target/<profile>/deps) — same compiler, same ndata, offline, seconds.
//! They define a local `Initializer` layout-identical to
//! `flowlang::hotswap::Initializer`, exactly as crates scaffolded before
//! the canonical definition do in production.
//!
//! One #[test] fn on purpose: the DataStore/ndata globals initialize once
//! per process, so the phases run as one sequence in one process.

use std::path::{Path, PathBuf};

use flowlang::datastore::DataStore;
use flowlang::hotswap;
use flowlang::rustcmd::{RustCmd, Transform};
use ndata::dataobject::DataObject;

const FIXTURE_TEMPLATE: &str = r#"
use ndata::dataobject::DataObject;
use ndata::NDataConfig;

pub type Transform = fn(DataObject) -> DataObject;

// Layout-identical local copy of flowlang::hotswap::Initializer, as in
// crates scaffolded before the canonical definition existed.
#[repr(C)]
#[derive(Debug)]
pub struct Initializer {
    pub ndata_config: NDataConfig,
    pub cmds: Vec<(String, Transform, String)>,
}

static START: std::sync::Once = std::sync::Once::new();

fn plus(o: DataObject) -> DataObject {
    let mut r = DataObject::new();
    r.put_int("a", o.get_int("x") + __PLUS__);
    r
}

#[allow(dead_code)]
fn shout(o: DataObject) -> DataObject {
    let mut r = DataObject::new();
    r.put_string("a", &o.get_string("s").to_uppercase());
    r
}

#[no_mangle]
pub unsafe extern "C" fn mirror___ROOT__(initializer: *mut Initializer) {
    if initializer.is_null() { return; }
    START.call_once(|| { ndata::mirror((*initializer).ndata_config); });
    (*initializer).cmds.push(("hotswap.e2e.plus".to_string(), plus as Transform, "".to_string()));
    __SHOUT_PUSH__
}

static CONTRACT: &[u8] = b"__CONTRACT__\0";

#[no_mangle]
pub extern "C" fn nb_ffi_contract___ROOT__() -> *const std::os::raw::c_char {
    CONTRACT.as_ptr() as *const std::os::raw::c_char
}
"#;

const SHOUT_PUSH: &str = r#"(*initializer).cmds.push(("hotswap.e2e.shout".to_string(), shout as Transform, "".to_string()));"#;

fn deps_dir() -> PathBuf {
    let target = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target"));
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    target.join(profile).join("deps")
}

/// The ndata rlibs this build produced, newest first. Several can exist
/// (feature unification across old builds); the compile tries each.
fn ndata_rlibs() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(deps_dir())
        .expect("target deps dir must exist under cargo test")
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("libndata-") && n.ends_with(".rlib"))
                .unwrap_or(false)
        })
        .collect();
    v.sort_by_key(|p| {
        std::cmp::Reverse(
            std::fs::metadata(p)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
        )
    });
    assert!(!v.is_empty(), "no libndata-*.rlib under {:?}", deps_dir());
    v
}

/// Compiles one fixture dylib with rustc, linking the host's own ndata.
fn build_fixture(src_dir: &Path, root: &str, plus: i64, with_shout: bool, contract: &str, out: &Path) {
    let source = FIXTURE_TEMPLATE
        .replace("__ROOT__", root)
        .replace("__PLUS__", &plus.to_string())
        .replace("__SHOUT_PUSH__", if with_shout { SHOUT_PUSH } else { "" })
        .replace("__CONTRACT__", contract);
    let src = src_dir.join(format!("{}_{}.rs", root, plus));
    std::fs::write(&src, source).unwrap();
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();

    let mut last_err = String::new();
    for rlib in ndata_rlibs() {
        let output = std::process::Command::new("rustc")
            .args(["--edition", "2021", "--crate-type", "cdylib"])
            .arg("--extern")
            .arg(format!("ndata={}", rlib.display()))
            .arg("-L")
            .arg(format!("dependency={}", deps_dir().display()))
            .arg("-o")
            .arg(out)
            .arg(&src)
            .output()
            .expect("rustc must be runnable in a cargo test environment");
        if output.status.success() {
            return;
        }
        last_err = String::from_utf8_lossy(&output.stderr).into_owned();
    }
    panic!("fixture '{}' failed to compile against every ndata rlib:\n{}", root, last_err);
}

fn exec(id: &str, args: DataObject) -> DataObject {
    RustCmd::new(id).execute(args).unwrap()
}

fn args_x(x: i64) -> DataObject {
    let mut o = DataObject::new();
    o.put_int("x", x);
    o
}

#[test]
fn hotswap_full_mirror_handshake() {
    // Deterministic test: no background poller; loads are explicit.
    std::env::set_var("NEWBOUND_FFI_POLL_MS", "0");

    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    let inst = std::env::temp_dir().join(format!("hotswap_e2e_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&inst);

    // A minimal instance: a store declaring two FFI-rooted libraries, and
    // the crates' dylibs where cargo would put them.
    let data = inst.join("data");
    std::fs::create_dir_all(data.join("hotdemo")).unwrap();
    std::fs::write(
        data.join("hotdemo").join("meta.json"),
        r#"{"root":"fixture","cargo":{"ffi":true}}"#,
    )
    .unwrap();
    std::fs::create_dir_all(data.join("badlib")).unwrap();
    std::fs::write(
        data.join("badlib").join("meta.json"),
        r#"{"root":"badfix","cargo":{"ffi":true}}"#,
    )
    .unwrap();

    let good_contract = flowlang::hotswap::contract().to_string();
    let src_dir = inst.join("fixture-src");
    std::fs::create_dir_all(&src_dir).unwrap();
    let fixture_so = inst.join("fixture").join("target").join(profile).join("libfixture.so");
    let badfix_so = inst.join("badfix").join("target").join(profile).join("libbadfix.so");

    // Generation 1: plus adds 1, shout exists, contract matches the host.
    build_fixture(&src_dir, "fixture", 1, true, &good_contract, &fixture_so);
    // badfix carries a deliberately wrong contract.
    build_fixture(&src_dir, "badfix", 1, false, "garbage-contract", &badfix_so);

    // Boot exactly as a host does: init the store, then hotswap::start.
    let dir_static: &'static str =
        Box::leak(data.to_string_lossy().into_owned().into_boxed_str());
    let magic = flowlang::init(dir_static);
    hotswap::start(magic);

    // start()'s rescan discovered 'fixture' from the store and loaded it;
    // 'badfix' was refused by the ABI contract and must not be live.
    let loaded = hotswap::loaded();
    assert!(loaded.iter().any(|(r, _)| r == "fixture"), "fixture not loaded: {:?}", loaded);
    assert!(!loaded.iter().any(|(r, _)| r == "badfix"), "badfix must be refused");
    let refusal = hotswap::load("badfix").unwrap_err();
    assert!(refusal.contains("ABI contract mismatch"), "unexpected error: {}", refusal);

    // Commands execute through the shared ndata heap.
    assert!(RustCmd::exists("hotswap.e2e.plus"));
    assert!(RustCmd::exists("hotswap.e2e.shout"));
    assert_eq!(exec("hotswap.e2e.plus", args_x(41)).get_int("a"), 42);
    let mut s = DataObject::new();
    s.put_string("s", "abc");
    assert_eq!(exec("hotswap.e2e.shout", s).get_string("a"), "ABC");

    // Capture generation 1's raw transform pointer, as RUST_COMMANDS holds it.
    let gen1_ptr = DataStore::globals()
        .get_object("RUST_COMMANDS")
        .get_object("hotswap.e2e.plus")
        .get_int("transform_ptr");

    // Generation 2: plus adds 100, shout is deleted.
    build_fixture(&src_dir, "fixture", 100, false, &good_contract, &fixture_so);
    hotswap::reload("fixture").unwrap();

    // New behavior is live, the deleted command fails clean (stale-id sweep).
    assert_eq!(exec("hotswap.e2e.plus", args_x(1)).get_int("a"), 101);
    assert!(!RustCmd::exists("hotswap.e2e.shout"));
    let g = hotswap::loaded();
    let fixture_gen = g.iter().find(|(r, _)| r == "fixture").unwrap().1;
    assert!(fixture_gen >= 2, "reload must advance the generation: {:?}", g);

    // Never-unload: generation 1's code is still mapped and still behaves
    // like generation 1 — this is what fixes the historical crash on
    // hot-swap of a library with code running in a thread.
    let gen1_fn: Transform = unsafe { std::mem::transmute(gen1_ptr) };
    assert_eq!(gen1_fn(args_x(1)).get_int("a"), 2);

    // A root whose dylib does not exist errors instead of panicking.
    assert!(hotswap::load("no-such-root").is_err());

    let _ = std::fs::remove_dir_all(&inst);
}
