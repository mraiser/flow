//! This file is the public facade for the flowlang builder system.
//!
//! It maintains 100% backward compatibility with the original build script's
//! public API. The actual implementation logic has been moved into the private
//! `builder` module, which is declared in the crate's `lib.rs`. This file simply
//! delegates all public function calls to it.

use std::path::Path;
use crate::DataStore;

/// Public entry point for a full project build.
/// Delegates to the internal builder implementation.
pub fn build_all() -> bool {
    crate::builder::build_all()
}

/// Public entry point for building a single library.
/// Delegates to the internal builder implementation.
pub fn build_lib(lib_name: String) -> bool {
    crate::builder::build_lib(lib_name)
}

/// Public entry point for building a single command.
///
/// This function maintains the original public signature. To call the new
/// internal `builder::build` function, it must first determine the correct
//  base path for the library's crate.
pub fn build(
    lib_name: &str,
    control_name: &str,
    command_name: &str,
    _library_data_path_arg: &Path, // This argument is ignored for backward compatibility.
) -> bool {
    let store = DataStore::new();
    let lib_metadata = store.lib_info(lib_name);

    // Determine the crate's root directory from metadata.
    let (lib_config_root_field, _) = crate::builder::util::get_crate_info(&lib_metadata);
    let library_build_base_path = if lib_config_root_field == "." {
        crate::builder::util::get_project_top_level_path()
    } else {
        crate::builder::util::get_project_top_level_path().join(&lib_config_root_field)
    };

    // Delegate to the internal builder.
    crate::builder::build(lib_name, control_name, command_name, &library_build_base_path)
}

/// Public entry point for rebuilding the Rust API.
/// Delegates to the internal builder implementation.
pub fn rebuild_rust_api() {
    crate::builder::rebuild_rust_api();
}
