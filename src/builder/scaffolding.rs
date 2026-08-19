//! This file is responsible for creating and maintaining the basic
//! file and directory structure for user crates (e.g., src/lib.rs, src/cmdinit.rs).

use std::fs::{create_dir_all, read_to_string, OpenOptions};
use std::io::Write;
use std::path::Path;

/// Ensures that the core `src/lib.rs` and `src/cmdinit.rs` files exist for a crate.
pub(crate) fn ensure_crate_files_exist(crate_src_path: &Path, is_ffi: bool) {
    if !crate_src_path.exists() {
        create_dir_all(crate_src_path).expect("Failed to create src directory for crate.");
    }

    let lib_rs_path = crate_src_path.join("lib.rs");
    if !lib_rs_path.exists() {
        let mut content = String::from(
r#"// This file is auto-generated and managed by the flowlang build script.
use flowlang::rustcmd::{Transform};

// Each flowlang library within this crate will be added as a module here.

mod cmdinit;
pub use cmdinit::cmdinit;
mod api;
pub static API : crate::api::api = crate::api::new();
"#);
        if is_ffi {
            let crate_name = crate_src_path.parent().and_then(|p| p.file_name()).and_then(|s| s.to_str()).unwrap_or("unknown_crate").replace("-", "_");

            // The handshake struct is canonical in flowlang::hotswap — both
            // sides of the FFI boundary name the same definition, so the
            // layouts can only drift if the flowlang versions drift, which
            // the contract symbol below turns into a load-time refusal.
            let ffi_content = format!(
r#"
use std::sync::Once;
use flowlang::hotswap::Initializer;

static START: Once = Once::new();

#[no_mangle]
pub unsafe extern "C" fn mirror_{0}(initializer: *mut Initializer) {{
    if initializer.is_null() {{ return; }}

    // Mirror the host's ndata heaps exactly once per loaded generation of
    // this library, however many times the host calls in.
    START.call_once(|| {{
        flowlang::mirror(("data", (*initializer).ndata_config));
    }});

    // Register this library's commands on every call, so a reload picks
    // up new and changed ones.
    cmdinit(&mut (*initializer).cmds);
}}

// ABI guard: the host compares this against its own contract before
// calling mirror, and refuses the load when the two flowlang copies
// disagree about the handshake.
#[no_mangle]
pub extern "C" fn nb_ffi_contract_{0}() -> *const std::os::raw::c_char {{
    flowlang::hotswap::contract_ptr()
}}
"#, crate_name);
            content.push_str(&ffi_content);
        }
        std::fs::write(&lib_rs_path, content).expect("Failed to write default lib.rs for crate.");
    }

    let cmdinit_rs_path = crate_src_path.join("cmdinit.rs");
    ensure_mod_file_has_cmdinit(&cmdinit_rs_path);
}

/// Creates the directory and `mod.rs` for a given control if they don't exist.
pub(crate) fn ensure_control_scaffolding(lib_src_path: &Path, control_name: &str) {
    let control_mod_path = lib_src_path.join(control_name);
    create_dir_all(&control_mod_path).expect("Failed to create control module directory");
    ensure_mod_file_has_cmdinit(&control_mod_path.join("mod.rs"));
}

/// Ensures a module file (like `mod.rs` or `cmdinit.rs`) exists and contains a `cmdinit` function stub.
pub(crate) fn ensure_mod_file_has_cmdinit(path: &Path) {
    if !path.exists() {
        let content = r#"// This file is auto-generated and managed by the flowlang build script.
use flowlang::rustcmd::Transform;
pub fn cmdinit(cmds: &mut Vec<(String, Transform, String)>) {
}
"#;
        if let Some(parent) = path.parent() {
            create_dir_all(parent).expect("Failed to create parent directory for mod.rs");
        }
        std::fs::write(path, content).expect("Failed to write default mod.rs with cmdinit");
    } else {
        let content = read_to_string(path).unwrap_or_default();
        if !content.contains("pub fn cmdinit") {
            // The closing brace must sit on its own line: update_mod_file_content
            // inserts registration lines by searching for a lone "}", and a
            // one-line "{}" stub leaves it appending them OUTSIDE the fn.
            let mut file = OpenOptions::new().append(true).open(path).unwrap();
            writeln!(file, "\npub fn cmdinit(cmds: &mut Vec<(String, flowlang::rustcmd::Transform, String)>) {{\n}}")
                .expect("Failed to append cmdinit to existing mod file");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ensure_crate_files_exist;

    #[test]
    fn ffi_scaffold_uses_canonical_initializer_and_contract() {
        let dir = std::env::temp_dir().join(format!("scaffold_test_ffi_{}", std::process::id()));
        let src = dir.join("hot-demo").join("src");
        let _ = std::fs::remove_dir_all(&dir);

        ensure_crate_files_exist(&src, true);
        let lib_rs = std::fs::read_to_string(src.join("lib.rs")).unwrap();

        // The handshake struct is named from flowlang, never redefined.
        assert!(lib_rs.contains("use flowlang::hotswap::Initializer;"));
        assert!(!lib_rs.contains("pub struct Initializer"));
        // Hyphens normalize into the symbol names.
        assert!(lib_rs.contains("pub unsafe extern \"C\" fn mirror_hot_demo"));
        assert!(lib_rs.contains("pub extern \"C\" fn nb_ffi_contract_hot_demo"));
        assert!(lib_rs.contains("flowlang::hotswap::contract_ptr()"));
        assert!(lib_rs.contains("flowlang::mirror((\"data\", (*initializer).ndata_config));"));

        // Scaffolding never rewrites an existing lib.rs.
        ensure_crate_files_exist(&src, true);
        assert_eq!(std::fs::read_to_string(src.join("lib.rs")).unwrap(), lib_rs);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn static_scaffold_carries_no_ffi_exports() {
        let dir = std::env::temp_dir().join(format!("scaffold_test_static_{}", std::process::id()));
        let src = dir.join("plain").join("src");
        let _ = std::fs::remove_dir_all(&dir);

        ensure_crate_files_exist(&src, false);
        let lib_rs = std::fs::read_to_string(src.join("lib.rs")).unwrap();
        assert!(!lib_rs.contains("mirror_"));
        assert!(!lib_rs.contains("nb_ffi_contract_"));
        assert!(!lib_rs.contains("hotswap"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
