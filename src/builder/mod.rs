//! This file serves as the internal entry point and orchestrator for the builder module.
//! It contains the crate-public functions that are called by the `buildrust.rs` facade.

// Declare the submodules as crate-public
// (api is fully public: external harnesses generate against arbitrary stores
// via `generate_api_code` and compile-check the result)
pub mod api;
pub(crate) mod cargo;
pub(crate) mod initializer;
pub(crate) mod loader;
pub(crate) mod python;
pub(crate) mod rust;
pub(crate) mod scaffolding;
pub(crate) mod util;

use std::collections::HashMap;
use std::fs::{create_dir_all, read_dir};
use std::path::Path;

use crate::DataStore;

/// The main entry point for a full project build.
pub(crate) fn build_all() -> bool {
    let mut overall_build_occurred = false;
    let store = DataStore::new();
    let lib_entries = read_dir("data")
        .expect("Failed to read 'data' directory. This directory is essential for the build process.");

    let mut crates_to_wire: HashMap<String, bool> = HashMap::new();
    let mut lib_roots: Vec<(String, String)> = Vec::new();

    for dir_entry_result in lib_entries {
        let dir_entry = dir_entry_result.expect("Error reading a directory entry in 'data'.");
        let lib_name = dir_entry.file_name().into_string().expect("Library name is not valid UTF-8.");

        let lib_metadata = store.lib_info(&lib_name);
        let (root_name, is_ffi) = self::util::get_crate_info(&lib_metadata);

        if root_name != "." {
             crates_to_wire.insert(root_name.clone(), is_ffi);
        }
        lib_roots.push((lib_name.clone(), root_name));

        if build_lib(lib_name) {
            overall_build_occurred = true;
        }
    }

    // Sweep stale per-library modules out of the OTHER crates' src trees.
    // A library whose root changed - e.g. a blanked root routed it into the
    // default `cmd` crate before being restored - otherwise keeps compiling
    // (and failing) from the old location forever, because the mod files are
    // append-only. Only directories named after a known library whose declared
    // root is elsewhere are touched; unknown dirs (runtime/, hand-placed
    // modules) are left alone. The top-level "." root is never swept.
    let mut roots_to_sweep: Vec<String> = crates_to_wire.keys().cloned().collect();
    if !roots_to_sweep.iter().any(|r| r == "cmd") {
        roots_to_sweep.push("cmd".to_string());
    }
    for crate_root in &roots_to_sweep {
        let crate_src = self::util::get_project_top_level_path().join(crate_root).join("src");
        for (lib_name, lib_root) in &lib_roots {
            if lib_root == crate_root {
                continue;
            }
            let stale_dir = crate_src.join(lib_name);
            if stale_dir.exists() {
                let _ = std::fs::remove_dir_all(&stale_dir);
                self::util::remove_mod_file_content(
                    &crate_src.join("lib.rs"),
                    &format!("pub mod {};", lib_name),
                    None,
                );
                self::util::remove_mod_file_content(
                    &crate_src.join("cmdinit.rs"),
                    &format!("{}::cmdinit(cmds);", lib_name),
                    Some(&format!("use crate::{};", lib_name)),
                );
                println!(
                    "Removed stale module '{}' from crate '{}' (its root is '{}')",
                    lib_name, crate_root, lib_root
                );
                overall_build_occurred = true;
            }
        }
    }

    self::initializer::generate_main_initializer(&crates_to_wire);
    overall_build_occurred
}

/// Builds a single library.
pub(crate) fn build_lib(lib_name: String) -> bool {
    let mut build_actions_performed = false;
    let store = DataStore::new();

    let lib_metadata = store.lib_info(&lib_name);
    let (lib_config_root_field, is_ffi) = self::util::get_crate_info(&lib_metadata);

    let library_build_base_path = if lib_config_root_field == "." {
        self::util::get_project_top_level_path()
    } else {
        self::util::get_project_top_level_path().join(&lib_config_root_field)
    };

    let crate_src_path = library_build_base_path.join("src");

    self::scaffolding::ensure_crate_files_exist(&crate_src_path, is_ffi);
    let lib_src_path = crate_src_path.join(&lib_name);
    create_dir_all(&lib_src_path).expect("Failed to create library module directory");
    self::scaffolding::ensure_mod_file_has_cmdinit(&lib_src_path.join("mod.rs"));

    self::util::update_mod_file_content(&crate_src_path.join("lib.rs"), &format!("pub mod {};", lib_name), None);
    self::util::update_mod_file_content(&crate_src_path.join("cmdinit.rs"), &format!("{}::cmdinit(cmds);", lib_name), Some(&format!("use crate::{};", lib_name)));

    if store.exists(&lib_name, "controls") {
        let controls_data = store.get_data(&lib_name, "controls");
        let controls_list = controls_data.get_object("data").get_array("list");

        for control_value in controls_list.objects() {
            let control_obj = control_value.object();
            let control_name = control_obj.get_string("name");
            let control_id = control_obj.get_string("id");

            // Use the new shared scaffolding function.
            self::scaffolding::ensure_control_scaffolding(&lib_src_path, &control_name);

            if store.exists(&lib_name, &control_id) {
                let control_file_data = store.get_data(&lib_name, &control_id);
                let data_section_for_control = control_file_data.get_object("data");

                if data_section_for_control.has("cmd") {
                    let commands_array = data_section_for_control.get_array("cmd");
                    for command_value_in_control in commands_array.objects() {
                        let command_obj_in_control = command_value_in_control.object();
                        let command_name_from_control = command_obj_in_control.get_string("name");
                        if build(
                            &lib_name,
                            &control_name,
                            &command_name_from_control,
                            &library_build_base_path,
                        ) {
                            build_actions_performed = true;
                        }
                    }
                }
            } else {
                 println!("No control file found for control ID: {} in library: {}", &control_id, &lib_name);
            }
        }
    }

    let cargo_toml_path = library_build_base_path.join("Cargo.toml");
    let package_name_for_default_cargo = if lib_config_root_field == "." { "main_project".to_string() } else { lib_config_root_field.clone() };
    let cargo_config_updates = if lib_metadata.has("cargo") { lib_metadata.get_object("cargo") } else { ndata::dataobject::DataObject::new() };

    if self::cargo::update_cargo_toml(&cargo_toml_path, &cargo_config_updates, &lib_name, &package_name_for_default_cargo, is_ffi) {
         build_actions_performed = true;
    }

    build_actions_performed
}

/// Builds a single command by dispatching to the correct language-specific builder.
pub(crate) fn build(
    lib_name: &str,
    control_name: &str,
    command_name: &str,
    library_build_base_path: &Path,
) -> bool {
    let store = DataStore::new();
    let command_id = store.lookup_cmd_id(lib_name, control_name, command_name);

    // --- FIX: Ensure scaffolding for the target control exists before building the command ---
    let lib_src_path = library_build_base_path.join("src").join(lib_name);
    self::scaffolding::ensure_control_scaffolding(&lib_src_path, control_name);
    // --- End Fix ---

    if !store.exists(lib_name, &command_id) {
        println!("Command ID {} not found for {}:{}:{}", command_id, lib_name, control_name, command_name);
        return false;
    }

    let command_metadata = store.get_data(lib_name, &command_id);
    let data_section = command_metadata.get_object("data");

    if !data_section.has("type") {
        println!("WARNING: Command definition for '{}:{}:{}' is missing 'type'.", lib_name, control_name, command_name);
        return false;
    }
    let command_type = data_section.get_string("type");
    let actual_library_build_src_path = library_build_base_path.join("src");

    match command_type.as_str() {
        "rust" => {
            self::rust::build_rust_command(&actual_library_build_src_path, lib_name, control_name, command_name, &store)
        }
        "python" => {
            self::python::build_python_command(&actual_library_build_src_path, lib_name, control_name, command_name, &store);
            false
        }
        _ => {
            println!("Unsupported command type '{}' for {}:{}:{}", command_type, lib_name, control_name, command_name);
            false
        }
    }
}

/// A crate-public facade for the API rebuilding logic.
pub(crate) fn rebuild_rust_api() {
    self::api::rebuild_rust_api();
}

/// Initializes the global store exactly once across ALL tests in this crate.
/// ndata's heap init is guarded but not atomic — two test threads racing
/// through `is_initialized()` both pass the check and the second `set` panics
/// ("SharedMutex may only be initialized once") — so every test that touches
/// ndata or the DataStore must come through this Once instead of calling
/// `ndata::init()` / `DataStore::init()` itself.
#[cfg(test)]
pub(crate) fn test_init() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        crate::DataStore::init("data");
    });
}
