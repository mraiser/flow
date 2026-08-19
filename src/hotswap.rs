//! Runtime loading and hot-reloading of FFI library crates.
//!
//! This module replaces the machinery the builder used to emit into every
//! host's `generated_initializer.rs` (libloading + notify + a hardcoded
//! crate list). The set of FFI crates is discovered from the store at
//! runtime — a library whose meta.json declares a non-"." `root` and
//! `cargo.ffi = true` — so a brand new FFI library goes live in a running
//! process with no initializer regeneration, host rebuild, or restart.
//!
//! The generated initializer's only job here is one call to [`start`]
//! after the static crates register. Platform commands make a compile
//! deterministic by calling [`load`]/[`reload`] directly after a
//! successful cargo build; a std-only mtime poller (no notify crates)
//! backstops out-of-band builds.
//!
//! Safety posture — **loaded libraries are never unloaded**. `RUST_COMMANDS`
//! holds raw fn pointers into loaded mappings indefinitely, and any thread
//! may be executing a transform when a reload lands; dlclose would unmap
//! code under a live instruction pointer (the historical crash on hot-swap
//! of a library with a running thread). Every generation therefore stays
//! mapped for the life of the process: one leaked mapping per reload,
//! bounded by reload count, dev-time only. A thread spawned by an old
//! generation keeps running that generation's code — safe, not upgraded.

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, Once, OnceLock};
use std::time::SystemTime;

use ndata::dataobject::DataObject;
use ndata::NDataConfig;

use crate::datastore::DataStore;
use crate::rustcmd::{RustCmd, Transform};

// ---------------------------------------------------------------------------
// The FFI handshake
// ---------------------------------------------------------------------------

/// The struct passed by pointer to each FFI crate's `mirror_<root>` export.
/// The crate mirrors the host's ndata heaps from `ndata_config`, then pushes
/// one `(command id, transform, io signature)` tuple per command into `cmds`.
///
/// **Field order and types are ABI** — never reorder, remove, or retype
/// fields. This is not a C ABI (`Vec`/`String`/fn pointers cross the
/// boundary); the real contract is *same rustc, same flowlang and ndata,
/// same profile on both sides*, checked at load time via [`contract`].
/// Crates scaffolded before this module carry a layout-identical local
/// definition; those must stay byte-for-byte compatible until regenerated.
#[repr(C)]
#[derive(Debug)]
pub struct Initializer {
    pub ndata_config: NDataConfig,
    pub cmds: Vec<(String, Transform, String)>,
}

type MirrorFn = unsafe extern "C" fn(*mut Initializer);
type ContractFn = unsafe extern "C" fn() -> *const c_char;

/// The host's ABI contract string. The scaffolded `nb_ffi_contract_<root>`
/// export returns the library's copy (via [`contract_ptr`], computed by the
/// library's own flowlang); [`load`] refuses the library when they differ.
/// Layout sizes are compared directly — stronger than version strings, and
/// dependency versions aren't visible to `env!` anyway.
pub fn contract() -> &'static str {
    static S: OnceLock<String> = OnceLock::new();
    S.get_or_init(|| {
        format!(
            "flowlang={};profile={};ptr={};init_size={};cfg_size={}",
            env!("CARGO_PKG_VERSION"),
            if cfg!(debug_assertions) { "debug" } else { "release" },
            std::mem::size_of::<usize>(),
            std::mem::size_of::<Initializer>(),
            std::mem::size_of::<NDataConfig>(),
        )
    })
}

/// Nul-terminated [`contract`] for the scaffolded `nb_ffi_contract_<root>`
/// export to return. Lives here so both sides of the boundary compute their
/// string with the same code.
pub fn contract_ptr() -> *const c_char {
    static C: OnceLock<CString> = OnceLock::new();
    C.get_or_init(|| CString::new(contract()).expect("contract string never contains a nul"))
        .as_ptr()
}

// ---------------------------------------------------------------------------
// DynLib — the libloading replacement
// ---------------------------------------------------------------------------

/// A minimal handle to a dynamically loaded library. Deliberately has **no
/// `Drop`**: this module's safety posture is never-unload (see the module
/// docs), so there is no dlclose path at all.
pub struct DynLib {
    handle: *mut std::ffi::c_void,
}

// The handle is a process-global token and nothing here ever unloads, so
// sharing it across threads is sound.
unsafe impl Send for DynLib {}
unsafe impl Sync for DynLib {}

#[cfg(unix)]
impl DynLib {
    /// Loads a library. RTLD_NOW resolves every symbol up front so a broken
    /// build fails here, not mid-transform; RTLD_LOCAL (the default) keeps
    /// generations from shadowing each other's symbols.
    pub fn open(path: &Path) -> Result<Self, String> {
        use std::os::unix::ffi::OsStrExt;
        let c_path = CString::new(path.as_os_str().as_bytes())
            .map_err(|_| format!("path contains a nul byte: {:?}", path))?;
        let handle = unsafe { libc::dlopen(c_path.as_ptr(), libc::RTLD_NOW) };
        if handle.is_null() {
            Err(format!("dlopen {:?} failed: {}", path, unsafe { dl_error() }))
        } else {
            Ok(DynLib { handle })
        }
    }

    /// Resolves a symbol as a value of type `T` (a fn pointer type).
    ///
    /// # Safety
    /// The caller asserts the symbol exists with exactly the signature `T`;
    /// a wrong signature is undefined behavior on the first call through it.
    pub unsafe fn get<T: Copy>(&self, symbol: &str) -> Result<T, String> {
        debug_assert_eq!(
            std::mem::size_of::<T>(),
            std::mem::size_of::<*mut std::ffi::c_void>(),
            "DynLib::get resolves pointer-sized symbols only"
        );
        let c_sym = CString::new(symbol)
            .map_err(|_| format!("symbol name contains a nul byte: {}", symbol))?;
        libc::dlerror(); // clear any stale error state
        let ptr = libc::dlsym(self.handle, c_sym.as_ptr());
        if ptr.is_null() {
            Err(format!("symbol '{}' not found: {}", symbol, dl_error()))
        } else {
            Ok(std::mem::transmute_copy(&ptr))
        }
    }
}

#[cfg(unix)]
unsafe fn dl_error() -> String {
    let e = libc::dlerror();
    if e.is_null() {
        "unknown dl error".to_string()
    } else {
        CStr::from_ptr(e).to_string_lossy().into_owned()
    }
}

// Windows support is kept dependency-free with hand-declared kernel32
// bindings. Best-effort: kept cfg-complete, but no Windows CI exercises it.
#[cfg(windows)]
mod win {
    #[link(name = "kernel32")]
    extern "system" {
        pub fn LoadLibraryW(lp_lib_file_name: *const u16) -> *mut std::ffi::c_void;
        pub fn GetProcAddress(
            h_module: *mut std::ffi::c_void,
            lp_proc_name: *const i8,
        ) -> *mut std::ffi::c_void;
        pub fn GetLastError() -> u32;
    }
}

#[cfg(windows)]
impl DynLib {
    pub fn open(path: &Path) -> Result<Self, String> {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let handle = unsafe { win::LoadLibraryW(wide.as_ptr()) };
        if handle.is_null() {
            Err(format!(
                "LoadLibraryW {:?} failed (GetLastError={})",
                path,
                unsafe { win::GetLastError() }
            ))
        } else {
            Ok(DynLib { handle })
        }
    }

    /// # Safety
    /// See the unix variant: `T` must be the symbol's true signature.
    pub unsafe fn get<T: Copy>(&self, symbol: &str) -> Result<T, String> {
        debug_assert_eq!(
            std::mem::size_of::<T>(),
            std::mem::size_of::<*mut std::ffi::c_void>(),
            "DynLib::get resolves pointer-sized symbols only"
        );
        let c_sym = CString::new(symbol)
            .map_err(|_| format!("symbol name contains a nul byte: {}", symbol))?;
        let ptr = win::GetProcAddress(self.handle, c_sym.as_ptr());
        if ptr.is_null() {
            Err(format!(
                "symbol '{}' not found (GetLastError={})",
                symbol,
                win::GetLastError()
            ))
        } else {
            Ok(std::mem::transmute_copy(&ptr))
        }
    }
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// (mtime, len) of a dylib file — the change detector the poller compares.
type FileStat = (SystemTime, u64);

struct LoadedLib {
    // Held for future symbol lookups; the mapping itself outlives even this
    // handle, because DynLib has no unload path.
    #[allow(dead_code)]
    lib: DynLib,
    /// Monotonic load counter across all roots; exposed via [`loaded`].
    generation: u64,
    stat: FileStat,
    registered_ids: Vec<String>,
}

static REGISTRY: OnceLock<Mutex<HashMap<String, LoadedLib>>> = OnceLock::new();
static CONFIG: OnceLock<NDataConfig> = OnceLock::new();
static GENERATION: AtomicU64 = AtomicU64::new(0);
// Serializes whole load operations; the registry mutex only guards map access.
static LOAD_LOCK: Mutex<()> = Mutex::new(());

fn registry() -> &'static Mutex<HashMap<String, LoadedLib>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

// ---------------------------------------------------------------------------
// Public surface
// ---------------------------------------------------------------------------

/// Called once from the generated initializer, after the static crates have
/// registered and with the tuple `flowlang::init` returned. Loads every FFI
/// crate the store declares whose dylib exists, then starts the poller.
/// Idempotent; a store with no FFI roots makes this a no-op scan.
pub fn start(magic: (&'static str, NDataConfig)) {
    let _ = CONFIG.set(magic.1);
    #[cfg(windows)]
    sweep_stale_copies();
    rescan();
    start_poller();
}

/// Loads (or first-loads) one crate root: copy the dylib to a unique temp
/// path, dlopen it, verify the ABI contract, call `mirror_<root>`, register
/// the returned commands. The explicit entry point for platform commands
/// (`activate_lib`, and `compile` after a successful FFI build).
pub fn load(root: &str) -> Result<(), String> {
    load_generation(root)
}

/// Loads a new generation of `root` and re-registers its commands: ids the
/// new build still declares are overwritten in place, ids it dropped are
/// removed from `RUST_COMMANDS` (stale-id sweep). The previous generation
/// stays mapped, so in-flight calls and threads on old code stay valid.
/// Same operation as [`load`]; the two names state intent at call sites.
pub fn reload(root: &str) -> Result<(), String> {
    load_generation(root)
}

/// The loaded roots with their current generation numbers — introspection
/// for tests, logs, and platform commands.
pub fn loaded() -> Vec<(String, u64)> {
    registry()
        .lock()
        .unwrap()
        .iter()
        .map(|(root, l)| (root.clone(), l.generation))
        .collect()
}

/// Re-reads the store's library metadata and loads any FFI root that is not
/// loaded yet but has a dylib on disk. Called by [`start`]; platform code
/// may call it after creating or importing a library.
pub fn rescan() {
    for root in ffi_roots() {
        let already = registry().lock().unwrap().contains_key(&root);
        if !already && stat_dylib(&root).is_some() {
            match load_generation(&root) {
                Ok(_) => {}
                Err(e) => eprintln!("hotswap: failed to load '{}': {}", root, e),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Store discovery and path resolution
// ---------------------------------------------------------------------------

/// Every distinct crate root the store declares as FFI. The root is the
/// load unit — multiple libraries may share one crate.
fn ffi_roots() -> Vec<String> {
    let store = DataStore::new();
    let mut roots: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&store.root) {
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let lib = match entry.file_name().into_string() {
                Ok(s) => s,
                Err(_) => continue,
            };
            let (root, is_ffi) = store.lib_crate_info(&lib);
            if is_ffi && root != "." && !roots.contains(&root) {
                roots.push(root);
            }
        }
    }
    roots
}

/// Where `root`'s dylib lands. Crate roots resolve exactly as
/// `DataStore::get_lib_root` resolves them: absolute paths are honored,
/// relative ones live beside the store.
fn dylib_path(root: &str) -> PathBuf {
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    let (prefix, ext) = if cfg!(target_os = "windows") {
        ("", "dll")
    } else if cfg!(target_os = "macos") {
        ("lib", "dylib")
    } else {
        ("lib", "so")
    };
    // Cargo names the artifact after the lib target, which normalizes
    // hyphens to underscores.
    let stem = root.replace('-', "_");
    let base = if root.starts_with('/') {
        PathBuf::from(root)
    } else {
        let store = DataStore::new();
        match store.root.parent() {
            Some(p) => p.join(root),
            None => PathBuf::from(root),
        }
    };
    base.join("target")
        .join(profile)
        .join(format!("{}{}.{}", prefix, stem, ext))
}

fn stat_dylib(root: &str) -> Option<FileStat> {
    let md = std::fs::metadata(dylib_path(root)).ok()?;
    Some((md.modified().ok()?, md.len()))
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

fn load_generation(root: &str) -> Result<(), String> {
    let _serialized = LOAD_LOCK.lock().unwrap();

    let config = *CONFIG
        .get()
        .ok_or_else(|| "flowlang::hotswap::start has not been called".to_string())?;

    let src = dylib_path(root);
    let md = std::fs::metadata(&src)
        .map_err(|e| format!("library file not found at {:?}: {}", src, e))?;
    let stat: FileStat = (md.modified().map_err(|e| e.to_string())?, md.len());

    // Copy to a unique path so dlopen maps a fresh library instead of
    // handing back its cached mapping for a path it has seen before.
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let ext = src.extension().and_then(|s| s.to_str()).unwrap_or("so");
    let temp = std::env::temp_dir().join(format!(
        "nb_ffi_{}_{}_{}.{}",
        root.replace('-', "_"),
        std::process::id(),
        generation,
        ext
    ));
    std::fs::copy(&src, &temp)
        .map_err(|e| format!("failed to copy {:?} to {:?}: {}", src, temp, e))?;

    let lib = match DynLib::open(&temp) {
        Ok(l) => l,
        Err(e) => {
            let _ = std::fs::remove_file(&temp);
            return Err(e);
        }
    };

    // On unix the mapping holds its own reference to the file, so the copy
    // can go immediately and nothing accumulates in temp_dir. Windows locks
    // loaded DLLs; start() sweeps the leftovers of previous processes.
    #[cfg(unix)]
    let _ = std::fs::remove_file(&temp);

    let safe = root.replace('-', "_");

    // ABI contract: refuse a library whose flowlang disagrees about the
    // handshake layout. A missing symbol is a crate scaffolded before the
    // contract existed — warn and load (the check is advisory for legacy
    // crates, strict for everything that carries the symbol).
    if let Ok(contract_fn) = unsafe { lib.get::<ContractFn>(&format!("nb_ffi_contract_{}", safe)) }
    {
        let theirs_ptr = unsafe { contract_fn() };
        let theirs = if theirs_ptr.is_null() {
            String::new()
        } else {
            unsafe { CStr::from_ptr(theirs_ptr) }
                .to_string_lossy()
                .into_owned()
        };
        if theirs != contract() {
            return Err(format!(
                "refusing to load '{}': ABI contract mismatch (host '{}', library '{}')",
                root,
                contract(),
                theirs
            ));
        }
    } else {
        println!(
            "hotswap: library '{}' predates the ABI contract symbol; loading unchecked",
            root
        );
    }

    let mirror: MirrorFn = unsafe { lib.get(&format!("mirror_{}", safe)) }
        .map_err(|e| format!("cannot initialize '{}': {}", root, e))?;

    let mut initializer = Initializer {
        ndata_config: config,
        cmds: Vec::new(),
    };
    unsafe {
        mirror(&mut initializer as *mut Initializer);
    }

    let old_ids = registry()
        .lock()
        .unwrap()
        .get(root)
        .map(|l| l.registered_ids.clone())
        .unwrap_or_default();
    let registered_ids = register_commands(initializer.cmds, &old_ids);

    println!(
        "hotswap: loaded '{}' generation {} ({} commands)",
        root,
        generation,
        registered_ids.len()
    );
    registry().lock().unwrap().insert(
        root.to_string(),
        LoadedLib {
            lib,
            generation,
            stat,
            registered_ids,
        },
    );
    Ok(())
}

/// Writes a generation's commands into `RUST_COMMANDS` and sweeps the ids
/// the previous generation registered that this one no longer declares, so
/// a deleted command fails with "no such command" instead of silently
/// running leaked old code forever.
fn register_commands(cmds: Vec<(String, Transform, String)>, old_ids: &[String]) -> Vec<String> {
    let mut globals = DataStore::globals();
    if !globals.has("RUST_COMMANDS") {
        globals.put_object("RUST_COMMANDS", DataObject::new());
    }
    let mut cmd_map = globals.get_object("RUST_COMMANDS");

    let mut new_ids = Vec::with_capacity(cmds.len());
    for (id, transform, io) in cmds {
        let detail = RustCmd::detail(id.clone(), transform, io);
        cmd_map.put_object(&id, detail);
        new_ids.push(id);
    }

    for id in old_ids {
        if !new_ids.contains(id) && cmd_map.has(id) {
            cmd_map.remove_property(id);
            println!("hotswap: removed stale command '{}'", id);
        }
    }

    new_ids
}

// ---------------------------------------------------------------------------
// The poller — the notify/debouncer replacement
// ---------------------------------------------------------------------------

/// One poll decision for one root. Returns (next quiesce record, act now).
/// A (re)load fires only for a stat that differs from the loaded
/// generation's AND was already seen unchanged on the previous tick — the
/// two-tick quiesce that keeps a dylib cargo is still writing from loading.
fn poll_step(
    loaded: Option<FileStat>,
    pending: Option<FileStat>,
    current: FileStat,
) -> (Option<FileStat>, bool) {
    if loaded == Some(current) {
        (None, false) // steady state
    } else if pending == Some(current) {
        (None, true) // quiesced across a tick: act
    } else {
        (Some(current), false) // first sighting: wait a tick
    }
}

fn start_poller() {
    let ms: u64 = std::env::var("NEWBOUND_FFI_POLL_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2000);
    if ms == 0 {
        println!("hotswap: poller disabled (NEWBOUND_FFI_POLL_MS=0)");
        return;
    }
    static STARTED: Once = Once::new();
    STARTED.call_once(|| {
        std::thread::Builder::new()
            .name("hotswap-poller".to_string())
            .spawn(move || {
                let mut pending: HashMap<String, FileStat> = HashMap::new();
                // A stat that failed to load is not retried until it changes,
                // so a broken build logs once instead of every other tick.
                let mut failed: HashMap<String, FileStat> = HashMap::new();
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(ms));
                    for root in ffi_roots() {
                        let current = match stat_dylib(&root) {
                            Some(s) => s,
                            None => {
                                pending.remove(&root);
                                continue;
                            }
                        };
                        if failed.get(&root) == Some(&current) {
                            continue;
                        }
                        let loaded = registry().lock().unwrap().get(&root).map(|l| l.stat);
                        let (next_pending, act) =
                            poll_step(loaded, pending.get(&root).copied(), current);
                        match next_pending {
                            Some(s) => {
                                pending.insert(root.clone(), s);
                            }
                            None => {
                                pending.remove(&root);
                            }
                        }
                        if act {
                            println!("hotswap: '{}' dylib changed; reloading", root);
                            if let Err(e) = load_generation(&root) {
                                eprintln!("hotswap: reload of '{}' failed: {}", root, e);
                                failed.insert(root.clone(), current);
                            } else {
                                failed.remove(&root);
                            }
                        }
                    }
                }
            })
            .expect("failed to spawn hotswap poller thread");
    });
}

/// Windows can't delete a loaded DLL, so temp copies survive the process
/// that made them; sweep the leftovers at startup (copies a live process
/// still holds are locked and skip harmlessly).
#[cfg(windows)]
fn sweep_stale_copies() {
    if let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with("nb_ffi_") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_names_the_load_bearing_facts() {
        let c = contract();
        assert!(c.contains("flowlang="));
        assert!(c.contains("profile="));
        assert!(c.contains("init_size="));
        // The nul-terminated variant round-trips to the same string.
        let p = contract_ptr();
        let back = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
        assert_eq!(back, c);
    }

    #[test]
    fn poll_step_two_tick_quiesce() {
        let t0 = (SystemTime::UNIX_EPOCH, 100u64);
        let t1 = (SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(5), 200u64);

        // Steady state: loaded matches disk — nothing pends, nothing acts.
        assert_eq!(poll_step(Some(t0), None, t0), (None, false));
        // A change is sighted: pend it, don't act yet.
        assert_eq!(poll_step(Some(t0), None, t1), (Some(t1), false));
        // Same stat again next tick: quiesced, act.
        assert_eq!(poll_step(Some(t0), Some(t1), t1), (None, true));
        // Still being written (stat moved between ticks): re-pend.
        assert_eq!(poll_step(Some(t0), Some(t0), t1), (Some(t1), false));
        // Not loaded at all: same quiesce protocol for first load.
        assert_eq!(poll_step(None, None, t1), (Some(t1), false));
        assert_eq!(poll_step(None, Some(t1), t1), (None, true));
    }

    #[test]
    fn crate_info_reads_root_and_ffi() {
        crate::builder::test_init();
        let m = DataObject::from_string(r#"{"root":"agent","cargo":{"ffi":true}}"#);
        assert_eq!(
            crate::datastore::crate_info_from_meta(&m),
            ("agent".to_string(), true)
        );
        let m = DataObject::from_string(r#"{"root":"","cargo":{"ffi":false}}"#);
        assert_eq!(
            crate::datastore::crate_info_from_meta(&m),
            ("cmd".to_string(), false)
        );
        let m = DataObject::from_string(r#"{"cargo":{}}"#);
        assert_eq!(
            crate::datastore::crate_info_from_meta(&m),
            ("cmd".to_string(), false)
        );
        let m = DataObject::from_string("{}");
        assert_eq!(
            crate::datastore::crate_info_from_meta(&m),
            ("cmd".to_string(), false)
        );
    }

    fn t_alpha(o: DataObject) -> DataObject {
        o
    }
    fn t_beta(o: DataObject) -> DataObject {
        o
    }

    #[test]
    fn register_overwrites_and_sweeps_stale_ids() {
        crate::builder::test_init();
        let gen1 = vec![
            ("hotswap.test.alpha".to_string(), t_alpha as Transform, "".to_string()),
            ("hotswap.test.beta".to_string(), t_beta as Transform, "".to_string()),
        ];
        let ids1 = register_commands(gen1, &[]);
        assert!(RustCmd::exists("hotswap.test.alpha"));
        assert!(RustCmd::exists("hotswap.test.beta"));

        // Generation 2 drops beta: alpha survives (overwritten in place),
        // beta is swept so it fails clean instead of running stale code.
        let gen2 = vec![(
            "hotswap.test.alpha".to_string(),
            t_beta as Transform,
            "".to_string(),
        )];
        let ids2 = register_commands(gen2, &ids1);
        assert!(RustCmd::exists("hotswap.test.alpha"));
        assert!(!RustCmd::exists("hotswap.test.beta"));
        assert_eq!(ids2, vec!["hotswap.test.alpha".to_string()]);
    }

    /// The core loader guarantees, proven against real dylibs compiled by
    /// the test itself (rustc only, no dependencies): two generations load
    /// side by side, and a pointer into generation 1 stays callable after
    /// generation 2 loads — never-unload is what fixes the historical crash
    /// on hot-swap of a library with code running in a thread.
    #[test]
    #[cfg(unix)]
    fn dynlib_generations_never_unload() {
        let dir = std::env::temp_dir().join(format!("hotswap_dynlib_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let build = |answer: i32, out: &Path| {
            let src = dir.join(format!("fix_{}.rs", answer));
            std::fs::write(
                &src,
                format!(
                    "#[no_mangle]\npub extern \"C\" fn fixture_answer() -> i32 {{ {} }}\n",
                    answer
                ),
            )
            .unwrap();
            let status = std::process::Command::new("rustc")
                .args(["--crate-type", "cdylib", "-o"])
                .arg(out)
                .arg(&src)
                .status()
                .expect("rustc must be runnable in a cargo test environment");
            assert!(status.success(), "fixture dylib failed to compile");
        };

        let so1 = dir.join("libfix1.so");
        let so2 = dir.join("libfix2.so");
        build(41, &so1);
        build(42, &so2);

        type AnswerFn = unsafe extern "C" fn() -> i32;
        let lib1 = DynLib::open(&so1).unwrap();
        let f1: AnswerFn = unsafe { lib1.get("fixture_answer") }.unwrap();
        assert_eq!(unsafe { f1() }, 41);

        let lib2 = DynLib::open(&so2).unwrap();
        let f2: AnswerFn = unsafe { lib2.get("fixture_answer") }.unwrap();
        assert_eq!(unsafe { f2() }, 42);

        // Generation 1 is still mapped and callable after generation 2
        // loaded — and stays so even after its handle is gone, because
        // DynLib has no unload path.
        assert_eq!(unsafe { f1() }, 41);
        drop(lib1);
        assert_eq!(unsafe { f1() }, 41);

        // A missing symbol reports, not crashes.
        assert!(unsafe { lib2.get::<AnswerFn>("no_such_symbol") }.is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
