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

            // This is the FFI-safe definition that passes the entire NDataConfig.
            let ffi_content = format!(
r#"
use std::sync::Once;

// THIS IS THE FFI-SAFE INITIALIZER STRUCT.
// ITS DEFINITION MUST EXACTLY MATCH THE ONE IN THE MAIN BINARY.
#[repr(C)]
#[derive(Debug)]
pub struct Initializer {{
    pub ndata_config: ndata::NDataConfig,
    pub cmds: Vec<(String, Transform, String)>,
}}

static START: Once = Once::new();

#[no_mangle]
pub unsafe extern "C" fn mirror_{}(initializer: *mut Initializer) {{
    if initializer.is_null() {{ return; }}

    // Use Once to ensure ndata::mirror is only ever called one time,
    // even across multiple hot-reloads of this library.
    START.call_once(|| {{
        flowlang::mirror(("data", (*initializer).ndata_config));
    }});

    // Then, call this library's internal cmdinit to populate the cmds vector.
    // We want this to run on every reload to register any new commands.
    cmdinit(&mut (*initializer).cmds);
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
            let mut file = OpenOptions::new().append(true).open(path).unwrap();
            writeln!(file, "\npub fn cmdinit(cmds: &mut Vec<(String, flowlang::rustcmd::Transform, String)>) {{}}")
                .expect("Failed to append cmdinit to existing mod file");
        }
    }
}
