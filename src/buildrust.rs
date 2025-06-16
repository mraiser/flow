use std::collections::{HashMap, HashSet}; // Added HashSet
use std::fs::{create_dir_all, read_dir, read_to_string, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use ndata::dataarray::DataArray;
use ndata::dataobject::DataObject;

use crate::DataStore;

// Helper to get the assumed project top-level directory.
fn get_project_top_level_path() -> PathBuf {
    std::env::current_dir().expect("Failed to get current directory, which is assumed to be project root.")
}


// --- Public API Functions ---

pub fn build_all() -> bool {
    let mut overall_build_occurred = false;
    let store = DataStore::new();
    let lib_entries = read_dir("data")
        .expect("Failed to read 'data' directory. This directory is essential for the build process.");

    let mut crates_to_wire: HashMap<String, bool> = HashMap::new();

    for dir_entry_result in lib_entries {
        let dir_entry = dir_entry_result
            .expect("Error reading a directory entry in 'data'. Check directory permissions and integrity.");
        let lib_name = dir_entry
            .file_name()
            .into_string()
            .expect("Library name is not valid UTF-8. All library names in 'data' must be valid UTF-8 strings.");

        let lib_metadata = store.lib_info(&lib_name);
        let (root_name, is_ffi) = get_crate_info(&lib_metadata);

        // Don't register the main project itself for wiring, it's the host.
        if root_name != "." {
             crates_to_wire.insert(root_name, is_ffi);
        }

        if build_lib(lib_name) {
            overall_build_occurred = true;
        }
    }

    // After all libraries are built, generate the single initializer for the main binary.
    generate_main_initializer(&crates_to_wire);

    overall_build_occurred
}

pub fn build_lib(lib_name: String) -> bool {
    let mut build_actions_performed = false;
    let store = DataStore::new();

    let lib_metadata = store.lib_info(&lib_name);
    let (lib_config_root_field, is_ffi) = get_crate_info(&lib_metadata);

    let library_build_base_path = if lib_config_root_field == "." {
        get_project_top_level_path()
    } else {
        get_project_top_level_path().join(&lib_config_root_field)
    };

    let crate_src_path = library_build_base_path.join("src");

    // Unconditionally create the module structure for every library.
    ensure_crate_files_exist(&crate_src_path, is_ffi);
    let lib_src_path = crate_src_path.join(&lib_name);
    create_dir_all(&lib_src_path).expect("Failed to create library module directory");
    ensure_mod_file_has_cmdinit(&lib_src_path.join("mod.rs"));

    update_mod_file_content(&crate_src_path.join("lib.rs"), &format!("pub mod {};", lib_name), None);
    update_mod_file_content(&crate_src_path.join("cmdinit.rs"), &format!("{}::cmdinit(cmds);", lib_name), Some(&format!("use crate::{};", lib_name)));

    if store.exists(&lib_name, "controls") {
        let controls_data = store.get_data(&lib_name, "controls");
        let controls_list = controls_data.get_object("data").get_array("list");

        for control_value in controls_list.objects() {
            let control_obj = control_value.object();
            let control_name = control_obj.get_string("name");
            let control_id = control_obj.get_string("id");

            if !store.exists(&lib_name, &control_id) {
                println!(
                    "No control file found for control ID: {} in library: {}",
                    &control_id, &lib_name
                );
            } else {
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
                            &PathBuf::new()
                        ) {
                            build_actions_performed = true;
                        }
                    }
                }
            }
        }
    }

    let cargo_toml_path = library_build_base_path.join("Cargo.toml");
    let package_name_for_default_cargo = if lib_config_root_field == "." {
        "main_project".to_string()
    } else {
        lib_config_root_field.clone()
    };

    let cargo_config_updates = if lib_metadata.has("cargo") {
        lib_metadata.get_object("cargo")
    } else {
        DataObject::new()
    };

    if update_cargo_toml(&cargo_toml_path, &cargo_config_updates, &lib_name, &package_name_for_default_cargo, is_ffi) {
         build_actions_performed = true;
    }

    build_actions_performed
}

pub fn build(
    lib_name: &str,
    control_name: &str,
    command_name: &str,
    _library_data_path_arg: &Path, // Ignored.
) -> bool {
    let mut artifact_changed = false;
    let store = DataStore::new();

    let lib_metadata = store.lib_info(lib_name);
    let (lib_config_root_field, _) = get_crate_info(&lib_metadata);

    let library_build_base_path = if lib_config_root_field == "." {
        get_project_top_level_path()
    } else {
        get_project_top_level_path().join(&lib_config_root_field)
    };
    let actual_library_build_src_path = library_build_base_path.join("src");

    let command_id = store.lookup_cmd_id(lib_name, control_name, command_name);

    if store.exists(lib_name, &command_id) {
        let command_metadata = store.get_data(lib_name, &command_id);
        let data_section = command_metadata.get_object("data");

        if !data_section.has("type") {
            println!(
                "WARNING: Command definition for '{}:{}:{}' (ID: {}) is missing the 'type' field. Skipping.",
                lib_name, control_name, command_name, command_id
            );
            return false;
        }
        let command_type = data_section.get_string("type");

        let command_and_control_output_path = actual_library_build_src_path.join(lib_name).join(control_name);
        if !command_and_control_output_path.exists() {
            create_dir_all(&command_and_control_output_path)
                .expect(&format!("Failed to create directory: {:?}", command_and_control_output_path));
        }

        match command_type.as_str() {
            "rust" => {
                let rust_file_id = data_section.get_string("rust");
                let mut rust_meta = store.get_data(lib_name, &rust_file_id);
                let source_file_path = store.get_data_file(lib_name, &(rust_file_id.clone() + ".rs"));
                let source_code = store.read_file(source_file_path);

                rust_meta.put_string("lib", lib_name);
                rust_meta.put_string("ctl", control_name);
                rust_meta.put_string("cmd", command_name);

                if build_rust_command_source(&command_and_control_output_path, rust_meta, &source_code) {
                    artifact_changed = true;
                }
                build_mod_files_for_rust_command(
                    &command_and_control_output_path,
                    &lib_name,
                    &control_name,
                    &command_name,
                    &rust_file_id
                );
            }
            "python" => {
                let python_file_id = data_section.get_string("python");
                let mut python_meta = store.get_data(lib_name, &python_file_id);
                let source_file_path = store.get_data_file(lib_name, &(python_file_id.clone() + ".python"));
                let source_code = store.read_file(source_file_path);

                python_meta.put_string("lib", lib_name);
                python_meta.put_string("ctl", control_name);
                python_meta.put_string("cmd", command_name);

                build_python_command_source(&command_and_control_output_path, python_meta, &source_code);
            }
            _ => {
                println!("Unsupported command type '{}' for {}:{}:{}", command_type, lib_name, control_name, command_name);
            }
        }
    } else {
        println!(
            "Command ID {} not found for {}:{}:{}",
            command_id, lib_name, control_name, command_name
        );
    }
    artifact_changed
}

// --- Build Logic Refactoring ---

fn generate_main_initializer(crates: &HashMap<String, bool>) {
    let top_level_path = get_project_top_level_path();
    let generated_file_path = top_level_path.join("src").join("generated_initializer.rs");

    let mut main_init_lines: Vec<String> = vec![
        "// This file is auto-generated by the flowlang build script. Do not edit.".to_string(),
        "use flowlang::rustcmd::{RustCmd, Transform};".to_string(),
        "use ndata::NDataConfig;".to_string(),
        "use flowlang::datastore::DataStore;".to_string(),
    ];

    for (crate_name, &is_ffi) in crates.iter() {
        let safe_crate_name = crate_name.replace("-", "_");
        if is_ffi {
            // FFI crates need a unique mirror function. The Initializer struct is also defined here
            // because only FFI crates use it.
            main_init_lines.push(format!("\n// FFI Linker module for library: {}", crate_name));
            main_init_lines.push("#[derive(Debug, Clone)]".to_string());
            main_init_lines.push("pub struct Initializer {".to_string());
            main_init_lines.push("    pub data_ref: (&'static str, NDataConfig),".to_string());
            main_init_lines.push("    pub cmds: Vec<(String, Transform, String)>,".to_string());
            main_init_lines.push("}".to_string());
            main_init_lines.push(format!("mod {}_ffi {{", safe_crate_name));
            main_init_lines.push("    use super::Initializer;".to_string());
            main_init_lines.push(format!("    #[link(name = \"{}\", kind = \"dylib\")]", crate_name));
            main_init_lines.push("    extern \"C\" {".to_string());
            main_init_lines.push(format!("        pub fn mirror_{}(state: *mut Initializer);", safe_crate_name));
            main_init_lines.push("    }".to_string());
            main_init_lines.push("}".to_string());
        } else {
            main_init_lines.push(format!("use {};", safe_crate_name));
        }
    }

    main_init_lines.push("\npub fn initialize_all_commands(magic:(&'static str, NDataConfig)) {".to_string());
    main_init_lines.push("    let mut globals = DataStore::globals();".to_string());
    main_init_lines.push("    if !globals.has(\"RUST_COMMANDS\") {".to_string());
    main_init_lines.push("        globals.put_object(\"RUST_COMMANDS\", ndata::dataobject::DataObject::new());".to_string());
    main_init_lines.push("    }".to_string());
    main_init_lines.push("    let mut cmd_map = globals.get_object(\"RUST_COMMANDS\");".to_string());


    for (crate_name, &is_ffi) in crates.iter() {
        let safe_crate_name = crate_name.replace("-", "_");
        main_init_lines.push(format!("\n    // Initialize crate: {}", crate_name));
        main_init_lines.push("    {".to_string());
        main_init_lines.push("        let mut cmds = Vec::new();".to_string());

        if is_ffi {
            main_init_lines.push("        let mut initializer = Initializer {".to_string());
            main_init_lines.push("            data_ref: magic,".to_string());
            main_init_lines.push("            cmds: Vec::new(),".to_string());
            main_init_lines.push("        };".to_string());
            main_init_lines.push("        unsafe {".to_string());
            main_init_lines.push(format!("            {}_ffi::mirror_{}(&mut initializer as *mut _);", safe_crate_name, safe_crate_name));
            main_init_lines.push("        }".to_string());
            main_init_lines.push("        cmds.extend(initializer.cmds);".to_string());
        } else {
            main_init_lines.push(format!("        {}::cmdinit(&mut cmds);", safe_crate_name));
        }

        main_init_lines.push("        for q in cmds {".to_string());
        main_init_lines.push("            let cmd_details = RustCmd::detail(q.0.to_owned(), q.1, q.2.to_owned());".to_string());
        main_init_lines.push("            cmd_map.put_object(&q.0, cmd_details);".to_string());
        main_init_lines.push("        }".to_string());
        main_init_lines.push("    }".to_string());
    }

    main_init_lines.push("}".to_string());

    write_lines_to_file(&generated_file_path, &main_init_lines)
        .expect("Failed to write generated initializer file.");

    let main_cargo_path = top_level_path.join("Cargo.toml");
    if main_cargo_path.exists() {
        let mut lines = read_lines_from_file(&main_cargo_path).expect("Failed to read main Cargo.toml");
        let mut dependencies_map = HashMap::new();
        let dep_insertion_line = find_section_insertion_line(&lines, "[dependencies]", &mut dependencies_map);

        let mut new_deps_config = DataObject::new();
        for crate_name in crates.keys() {
            if !dependencies_map.contains_key(crate_name) {
                let dependency_value = format!("{{ path = \"./{}\" }}", crate_name);
                new_deps_config.put_string(crate_name, &dependency_value);
            }
        }

        if new_deps_config.clone().keys().len() > 0 {
             let (modified, _) = update_cargo_section_lines(
                &mut lines,
                &new_deps_config,
                &mut dependencies_map,
                dep_insertion_line,
                "Dependency",
                "main project's Cargo.toml",
            );

            if modified {
                write_lines_to_file(&main_cargo_path, &lines)
                    .expect("Failed to write updated main Cargo.toml with FFI dependency");
            }
        }
    }
}

fn build_mod_files_for_rust_command(
    command_output_path: &Path,
    #[allow(unused_variables)]
    lib_name: &str,
    control_name: &str,
    command_name: &str,
    rust_cmd_meta_id: &str,
) {
    let ctl_mod_file = command_output_path.join("mod.rs");
    ensure_mod_file_has_cmdinit(&ctl_mod_file);
    update_mod_file_content(&ctl_mod_file, &format!("pub mod {};", command_name), None);
    update_mod_file_content(&ctl_mod_file, &format!("cmds.push((\"{}\".to_string(), {}::execute, \"\".to_string()));", rust_cmd_meta_id, command_name), None);

    let lib_mod_path = command_output_path.parent()
        .expect("Command output path should have a parent (library module level)");
    let lib_mod_file = lib_mod_path.join("mod.rs");
    ensure_mod_file_has_cmdinit(&lib_mod_file);
    update_mod_file_content(&lib_mod_file, &format!("pub mod {};", control_name), None);
    update_mod_file_content(&lib_mod_file, &format!("{}::cmdinit(cmds);", control_name), None);
}

fn get_crate_info(lib_metadata: &DataObject) -> (String, bool) {
    let root = if lib_metadata.has("root") {
        let value = lib_metadata.get_string("root");
        if value.is_empty() { "cmd".to_string() } else { value }
    } else {
        "cmd".to_string()
    };

    let is_ffi = if lib_metadata.has("cargo") {
        let cargo_obj = lib_metadata.get_object("cargo");
        if cargo_obj.has("ffi") {
            cargo_obj.get_boolean("ffi")
        } else {
            false
        }
    } else {
        false
    };

    (root, is_ffi)
}

// COMPILE FIX: The `is_ffi` parameter is used to conditionally generate the `mirror` function.
fn ensure_crate_files_exist(crate_src_path: &Path, is_ffi: bool) {
    if !crate_src_path.exists() {
        create_dir_all(crate_src_path).expect("Failed to create src directory for crate.");
    }

    let lib_rs_path = crate_src_path.join("lib.rs");
    if !lib_rs_path.exists() {
        let mut content = String::from(r#"// This file is auto-generated and managed by the flowlang build script.
use flowlang::rustcmd::{Transform};

// Each flowlang library within this crate will be added as a module here.

mod cmdinit;
pub use cmdinit::cmdinit;
mod api;
pub static API : crate::api::api = crate::api::new();
"#);
        // Only FFI crates need the Initializer struct and a `mirror` function.
        if is_ffi {
            let ffi_content = format!(r#"
#[derive(Debug, Clone)]
pub struct Initializer {{
    pub cmds: Vec<(String, Transform, String)>,
}}

#[no_mangle]
pub fn mirror_{}(state: &mut Initializer) {{
    cmdinit(&mut state.cmds);
}}
"#, crate_src_path.parent().unwrap().file_name().unwrap().to_str().unwrap().replace("-", "_"));
            content.push_str(&ffi_content);
        }

        std::fs::write(&lib_rs_path, content).expect("Failed to write default lib.rs for crate.");
    }

    let cmdinit_rs_path = crate_src_path.join("cmdinit.rs");
    ensure_mod_file_has_cmdinit(&cmdinit_rs_path);
}

fn ensure_mod_file_has_cmdinit(path: &Path) {
    if !path.exists() {
        let content = r#"// This file is auto-generated and managed by the flowlang build script.
use flowlang::rustcmd::Transform;
pub fn cmdinit(cmds: &mut Vec<(String, Transform, String)>) {
}
"#;
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                create_dir_all(parent).expect("Failed to create parent directory for mod.rs");
            }
        }
        std::fs::write(path, content).expect("Failed to write default mod.rs with cmdinit");
    } else {
        let content = read_to_string(path).unwrap_or_default();
        if !content.contains("pub fn cmdinit") {
            let mut file = std::fs::OpenOptions::new().append(true).open(path).unwrap();
            writeln!(file, "\npub fn cmdinit(cmds: &mut Vec<(String, flowlang::rustcmd::Transform, String)>) {{}}")
                .expect("Failed to append cmdinit to existing mod file");
        }
    }
}


// --- Unchanged Helper Functions Below ---

pub fn rebuild_rust_api() {
    let store = DataStore::new();
    let project_top_level_path = get_project_top_level_path();

    let mut crates = HashSet::new();
    let lib_entries_for_crates = read_dir("data")
        .expect("Failed to read 'data' directory for API rebuilding.");
    for dir_entry_result in lib_entries_for_crates {
        let dir_entry = dir_entry_result.expect("Error reading a directory entry in 'data'.");
        let lib_name = dir_entry.file_name().into_string().expect("Library name is not valid UTF-8.");
        let lib_metadata = store.lib_info(&lib_name);
        let (root_name, _) = get_crate_info(&lib_metadata);
        if root_name != "." {
            crates.insert(root_name);
        }
    }

    let mut api_struct_init_str = "pub const fn new() -> api {\n    api {\n".to_string();
    let mut api_struct_def_str = "pub struct api {\n".to_string();
    let mut control_struct_defs_str = String::new();
    let mut command_wrapper_struct_defs_str = String::new();
    let mut impl_blocks_str = String::new();

    let lib_entries = read_dir("data")
        .expect("Failed to read 'data' directory for API rebuilding.");

    for db_result in lib_entries {
        let lib_entry = db_result.expect("Error reading library entry for API rebuilding.");
        let lib_name = lib_entry.file_name().into_string()
            .expect("Library name is not valid UTF-8 for API rebuilding.");

        if store.exists(&lib_name, "controls") {
            let safe_lib_name = lib_name.replace("-", "_");
            api_struct_init_str.push_str(&format!("        {}: {} {{\n", safe_lib_name, safe_lib_name));
            api_struct_def_str.push_str(&format!("    pub {}: {},\n", safe_lib_name, safe_lib_name));
            control_struct_defs_str.push_str(&format!("pub struct {} {{\n", safe_lib_name));

            let controls_data = store.get_data(&lib_name, "controls");
            let list = controls_data.get_object("data").get_array("list");

            for control_val in list.objects() {
                let control = control_val.object();
                let ctl_name = control.get_string("name");
                let safe_ctl_name = ctl_name.replace("-", "_");
                let ctl_id = control.get_string("id");

                if store.exists(&lib_name, &ctl_id) {
                    let struct_name = format!("{}_{}", safe_lib_name, safe_ctl_name);
                    api_struct_init_str.push_str(&format!("            {}: {} {{}},\n", safe_ctl_name, struct_name));
                    control_struct_defs_str.push_str(&format!("    pub {}: {},\n", safe_ctl_name, struct_name));
                    command_wrapper_struct_defs_str.push_str(&format!("pub struct {} {{}}\n", struct_name));

                    let ctldata = store.get_data(&lib_name, &ctl_id);
                    let d = ctldata.get_object("data");

                    if d.has("cmd") {
                        let cmdlist = d.get_array("cmd");
                        if cmdlist.len() > 0 {
                            impl_blocks_str.push_str(&format!("impl {} {{\n", struct_name));
                            for command_val in cmdlist.objects() {
                                let command = command_val.object();
                                let cmd_name = command.get_string("name");
                                let safe_cmd_name = cmd_name.replace("-", "_");
                                let cmd_id_in_control = command.get_string("id");

                                if store.exists(&lib_name, &cmd_id_in_control) {
                                    let meta_for_cmd_type = store.get_data(&lib_name, &cmd_id_in_control);
                                    let data_for_cmd_type = meta_for_cmd_type.get_object("data");
                                    let typ = if data_for_cmd_type.has("type") { data_for_cmd_type.get_string("type") } else { "java".to_string() };

                                    if typ == "rust" {
                                        let rust_meta_file_id = data_for_cmd_type.get_string("rust");
                                        impl_blocks_str.push_str(&format!("    pub fn {} (&self", safe_cmd_name));
                                        let mut params_str_for_fn_def = String::new();
                                        let mut params_setup_str_for_body = String::new();
                                        let rust_cmd_actual_meta = store.get_data(&lib_name, &rust_meta_file_id).get_object("data");
                                        let params_array = rust_cmd_actual_meta.get_array("params");

                                        for param_val in params_array.objects() {
                                            let param = param_val.object();
                                            let pname = param.get_string("name");
                                            let ptype = param.get_string("type");
                                            let dtype = lookup_rust_api_data_type(&ptype);
                                            let ntype = lookup_rust_api_ndata_method_suffix(&ptype);
                                            params_str_for_fn_def.push_str(&format!(", {}: {}", pname, dtype));
                                            let q = if dtype == "String" { "&" } else { "" };
                                            let method_prefix = if ntype == "property" { "set" } else { "put" };
                                            params_setup_str_for_body.push_str(&format!(
                                                "        d.{}_{}(\"{}\", {}{});\n",
                                                method_prefix, ntype, pname, q, pname
                                            ));
                                        }
                                        impl_blocks_str.push_str(&params_str_for_fn_def);
                                        let rtype_str = rust_cmd_actual_meta.get_string("returntype");
                                        let ntype_ret = lookup_rust_api_ndata_method_suffix(&rtype_str);
                                        let rtype_rust = lookup_rust_api_data_type(&rtype_str);
                                        impl_blocks_str.push_str(&format!(") -> {} {{\n", rtype_rust));

                                        if params_array.len() > 0 {
                                            impl_blocks_str.push_str("        let mut d = DataObject::new();\n");
                                        } else {
                                            impl_blocks_str.push_str("        let d = DataObject::new();\n");
                                        }
                                        impl_blocks_str.push_str(&params_setup_str_for_body);
                                        impl_blocks_str.push_str(&format!(
                                            "        RustCmd::new(\"{}\").execute(d).expect(\"Rust command execution failed\").get_{}(\"a\")\n    }}\n",
                                            rust_meta_file_id,
                                            ntype_ret
                                        ));
                                    }
                                }
                            }
                            impl_blocks_str.push_str("}\n");
                        }
                    }
                }
            }
            api_struct_init_str.push_str("        },\n");
            control_struct_defs_str.push_str("}\n");
        }
    }
    api_struct_init_str.push_str("    }\n}\n");
    api_struct_def_str.push_str("}");

    let use_statements = r#"#![allow(non_camel_case_types)]

use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use ndata::databytes::DataBytes;
use ndata::data::Data;
use flowlang::rustcmd::RustCmd;
"#;

    let final_api_code = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        use_statements,
        command_wrapper_struct_defs_str,
        control_struct_defs_str,
        api_struct_def_str,
        api_struct_init_str,
        impl_blocks_str
    );

    for crate_name in crates {
        let api_file_path = project_top_level_path.join(crate_name).join("src").join("api.rs");
        if let Some(parent_dir) = api_file_path.parent() {
            if !parent_dir.exists() {
                create_dir_all(parent_dir).expect(&format!("Failed to create directory for api.rs: {:?}", parent_dir));
            }
        }
        std::fs::write(&api_file_path, &final_api_code)
            .expect(&format!("Unable to write API file to {:?}", api_file_path));
    }
}

fn update_cargo_toml(cargo_toml_path: &PathBuf, cargo_config: &DataObject, lib_name: &str, default_package_name: &str, is_ffi: bool) -> bool {
    let mut file_was_created = false;
    if !cargo_toml_path.exists() {
        if default_package_name != "main_project" {
            println!("Cargo.toml not found at {:?} for sub-project '{}' (library {}), creating default.", cargo_toml_path, default_package_name, lib_name);

            let crate_types_str = if is_ffi {
                "[\"cdylib\", \"rlib\"]".to_string()
            } else if cargo_config.has("crate_types") {
                let crate_types_da = cargo_config.get_array("crate_types");
                let types: Vec<String> = crate_types_da.objects().iter()
                    .map(|val| val.string())
                    .collect();
                if !types.is_empty() {
                    format!("[{}]", types.iter().map(|s| format!("\"{}\"", s)).collect::<Vec<String>>().join(", "))
                } else {
                    "[\"rlib\"]".to_string()
                }
            } else {
                "[\"rlib\"]".to_string()
            };

            let default_content = format!(
r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = {}

[dependencies]
flowlang = {{ version = "0.3.25" }}
ndata = {{ version = "0.3.14" }}
serde = {{ version = "1.0", features = ["derive"], optional = true }}
serde_json = {{ version = "1.0", optional = true }}

[features]
reload = []
default = []
"# , default_package_name, crate_types_str);

            if let Some(parent_dir) = cargo_toml_path.parent() {
                if !parent_dir.exists() {
                    create_dir_all(parent_dir).expect("Failed to create parent directory for new Cargo.toml");
                }
            }
            std::fs::write(&cargo_toml_path, default_content)
                .expect(&format!("Failed to write default Cargo.toml to {:?}", cargo_toml_path));
            file_was_created = true;
        } else {
            println!("WARNING: Main project Cargo.toml not found at {:?}, cannot perform updates.", cargo_toml_path);
            return false;
        }
    }

    let mut config_caused_modification = false;
    let mut lines = read_lines_from_file(&cargo_toml_path)
        .expect(&format!("Failed to read Cargo.toml at {:?}", cargo_toml_path));

    if cargo_config.has("dependencies") {
        let mut dependencies_map = HashMap::new();
        let dependencies_insertion_line = find_section_insertion_line(&lines, "[dependencies]", &mut dependencies_map);
        let new_dependencies = cargo_config.get_object("dependencies");
        if new_dependencies.clone().keys().len() > 0 {
            if update_cargo_section_lines(
                &mut lines,
                &new_dependencies,
                &mut dependencies_map,
                dependencies_insertion_line,
                "Dependency",
                lib_name,
            ).0 {
                config_caused_modification = true;
            }
        }
    }

    if config_caused_modification {
        println!("Rewriting {}", cargo_toml_path.display());
        write_lines_to_file(&cargo_toml_path, &lines)
            .expect(&format!("Failed to write to Cargo.toml at {:?}", cargo_toml_path));
    }

    file_was_created || config_caused_modification
}

fn find_section_insertion_line(
    lines: &[String],
    section_marker: &str,
    existing_items_map: &mut HashMap<String, String>,
) -> usize {
    let mut section_start_idx: Option<usize> = None;
    let mut in_section = false;

    for (i, line_content) in lines.iter().enumerate() {
        let trimmed_line = line_content.trim();
        if trimmed_line == section_marker {
            section_start_idx = Some(i);
            in_section = true;
            existing_items_map.clear();
            continue;
        }

        if trimmed_line.starts_with('[') && in_section {
            break;
        }

        if in_section {
            if let Some(eq_offset) = trimmed_line.find('=') {
                let key = trimmed_line[..eq_offset].trim().to_string();
                let value_part = trimmed_line[eq_offset + 1..].trim();

                let final_value = if value_part.starts_with('"') && value_part.ends_with('"') && value_part.len() >= 2 {
                    value_part[1..value_part.len()-1].to_string()
                } else {
                    value_part.to_string()
                };
                existing_items_map.insert(key, final_value);
            }
        }
    }
    section_start_idx.map_or(lines.len(), |idx| idx + 1)
}

fn update_cargo_section_lines(
    cargo_lines: &mut Vec<String>,
    new_items_config: &DataObject,
    section_items_map: &mut HashMap<String, String>,
    mut current_insertion_idx: usize,
    item_type_name: &str,
    lib_name: &str,
) -> (bool, usize) {
    let mut section_modified = false;

    if current_insertion_idx > cargo_lines.len() {
        current_insertion_idx = cargo_lines.len();
    }
    let original_insertion_idx_for_updates = current_insertion_idx;

    for (key, value_obj) in new_items_config.objects() {
        let value_str = value_obj.string();
        let new_line_content: String;
        let trimmed_value_str = value_str.trim();

        if trimmed_value_str.starts_with('{') && trimmed_value_str.ends_with('}') {
            new_line_content = format!("{} = {}", key, value_str);
        } else if trimmed_value_str.starts_with('"') && trimmed_value_str.ends_with('"') && trimmed_value_str.len() >= 2 {
            new_line_content = format!("{} = {}", key, value_str);
        } else {
            new_line_content = format!("{} = \"{}\"", key, value_str);
        }

        if let Some(existing_file_value_raw) = section_items_map.get(&key) {
            let semantic_config_value = if trimmed_value_str.starts_with('"') && trimmed_value_str.ends_with('"') && trimmed_value_str.len() >=2 && !(trimmed_value_str.starts_with('{') && trimmed_value_str.ends_with('}')) {
                trimmed_value_str[1..trimmed_value_str.len()-1].to_string()
            } else {
                value_str.clone()
            };

            if existing_file_value_raw != &semantic_config_value {
                println!(
                    "WARNING: {} '{}' in Cargo.toml for library \"{}\" (current value: '{}') does not match config value ('{}'). Updating to: {}",
                    item_type_name, key, lib_name, existing_file_value_raw, semantic_config_value, new_line_content
                );
                let mut updated = false;
                for line_idx_to_update in original_insertion_idx_for_updates..cargo_lines.len() {
                    let trimmed_line_to_update = cargo_lines[line_idx_to_update].trim();
                    if trimmed_line_to_update.starts_with('[') && line_idx_to_update > original_insertion_idx_for_updates {
                        break;
                    }
                    if trimmed_line_to_update.starts_with(&format!("{} =", key)) ||
                       trimmed_line_to_update.starts_with(&format!("{}=", key)) ||
                       trimmed_line_to_update.starts_with(&format!("{} = ", key).trim_start())
                    {
                        cargo_lines[line_idx_to_update] = new_line_content.clone();
                        section_modified = true;
                        updated = true;
                        section_items_map.insert(key.clone(), semantic_config_value);
                        break;
                    }
                }
                 if !updated {
                     println!("WARNING: Could not find existing {} line for '{}' in Cargo.toml for library \"{}\" to update.", item_type_name, key, lib_name);
                 }
            }
        } else {
            println!("Adding new {} to Cargo.toml for library \"{}\": {}", item_type_name, lib_name, new_line_content);
            cargo_lines.insert(current_insertion_idx, new_line_content.clone());
            section_modified = true;
            let semantic_config_value_for_new = if trimmed_value_str.starts_with('"') && trimmed_value_str.ends_with('"') && trimmed_value_str.len() >=2 && !(trimmed_value_str.starts_with('{') && trimmed_value_str.ends_with('}')) {
                trimmed_value_str[1..trimmed_value_str.len()-1].to_string()
            } else {
                value_str.clone()
            };
            section_items_map.insert(key.clone(), semantic_config_value_for_new);
            current_insertion_idx += 1;
        }
    }
    (section_modified, current_insertion_idx)
}

fn read_lines_from_file(path: &PathBuf) -> Result<Vec<String>, std::io::Error> {
    let file = File::open(path)?;
    BufReader::new(file).lines().collect()
}

fn write_lines_to_file(path: &PathBuf, lines: &[String]) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            create_dir_all(parent).expect(&format!("Failed to create parent directory for {:?}", path));
        }
    }
    let mut file = File::create(path)?;
    for line in lines {
        writeln!(file, "{}", line)?;
    }
    Ok(())
}

fn find_line_index_in_slice(lines: &[String], marker: &str) -> Option<usize> {
    lines.iter().position(|line| line.trim() == marker.trim())
}

fn update_mod_file_content(
    mod_file_path: &PathBuf,
    line_to_add: &str,
    use_line_to_add: Option<&str>,
) {
    let mut lines = if mod_file_path.exists() {
        read_lines_from_file(mod_file_path)
            .expect(&format!("Failed to read mod file: {:?}", mod_file_path))
    } else {
        return;
    };

    let original_content = lines.join("\n");
    let mut modified = false;

    if let Some(use_line) = use_line_to_add {
        if !use_line.is_empty() && !original_content.contains(use_line) {
             let insert_at = lines.iter().rposition(|l| l.trim().starts_with("use ")).map_or(0, |i| i + 1);
             lines.insert(insert_at, use_line.to_string());
             modified = true;
        }
    }

    if !line_to_add.is_empty() && !original_content.contains(line_to_add) {
        if line_to_add.starts_with("pub mod") {
            let insert_at = lines.iter().rposition(|l| l.trim().starts_with("use ")).map_or(0, |i| i + 1);
            lines.insert(insert_at, line_to_add.to_string());
            modified = true;
        } else {
            if let Some(idx) = find_line_index_in_slice(&lines, "}") {
                 lines.insert(idx, format!("    {}", line_to_add));
                 modified = true;
            } else {
                 println!("WARNING: Could not find closing brace in {:?} to insert line: {}", mod_file_path, line_to_add);
                 lines.push(line_to_add.to_string());
                 modified = true;
            }
        }
    }

    if modified {
        write_lines_to_file(mod_file_path, &lines)
            .expect(&format!("Unable to write mod file: {:?}", mod_file_path));
    }
}

fn build_rust_command_source(
    output_path: &Path,
    meta: DataObject,
    template_code: &str,
) -> bool {
    let command_name = meta.get_string("cmd");
    let generated_src = generate_rust_source_from_meta(meta, template_code);
    let rust_output_file = output_path.join(format!("{}.rs", command_name));

    let mut needs_write = true;
    if rust_output_file.exists() {
        if let Ok(old_src) = read_to_string(&rust_output_file) {
            if old_src == generated_src {
                needs_write = false;
            }
        }
    }

    if needs_write {
        if let Some(parent) = rust_output_file.parent() {
             if !parent.exists() {
                create_dir_all(parent).expect("Failed to create directory for rust command source");
            }
        }
        std::fs::write(&rust_output_file, generated_src)
            .expect(&format!("Unable to write Rust file: {:?}", rust_output_file));
        return true;
    }
    false
}

fn map_meta_type_to_rust_type(meta_type: &str) -> String {
    match meta_type {
        "Any" => "Data", "Integer" => "i64", "Float" => "f64",
        "String" => "String", "File" => "String", "Boolean" => "bool",
        "JSONArray" => "DataArray", "InputStream" => "DataBytes",
        _ => "DataObject",
    }.to_string()
}

fn generate_rust_source_from_meta(meta: DataObject, user_code: &str) -> String {
    let data_section = meta.get_object("data");
    let command_name = meta.get_string("cmd");
    let user_provided_imports = data_section.get_string("import");
    let returntype_meta_str = data_section.get_string("returntype");
    let params_array = data_section.get_array("params");

    let mut src = String::new();

    let dataobject_import = "use ndata::dataobject::DataObject;";
    if !user_provided_imports.contains(dataobject_import) {
        src.push_str(dataobject_import);
        src.push('\n');
    }

    let mut wrapper_ndata_types_needed = HashSet::new();
    for param_value in params_array.objects() {
        let param_obj = param_value.object();
        let meta_type = param_obj.get_string("type");
        let rust_type = map_meta_type_to_rust_type(&meta_type);
        if ["DataArray", "DataBytes", "Data"].contains(&rust_type.as_str()) {
            wrapper_ndata_types_needed.insert(rust_type);
        }
    }
    let rust_return_type = map_meta_type_to_rust_type(&returntype_meta_str);
    if ["DataArray", "DataBytes", "Data"].contains(&rust_return_type.as_str()) {
        wrapper_ndata_types_needed.insert(rust_return_type.clone());
    }

    // COMPILE FIX: No longer conditionally add DataObject here.
    let ndata_types_to_import = [
        ("DataArray", "ndata::dataarray"),
        ("DataBytes", "ndata::databytes"),
        ("Data", "ndata::data"),
    ];

    for (type_name_str, module_path_str) in ndata_types_to_import.iter() {
        if wrapper_ndata_types_needed.contains(*type_name_str) {
            let use_line = format!("use {}::{};", module_path_str, type_name_str);
            if !user_provided_imports.contains(&use_line) {
                 src.push_str(&use_line);
                 src.push('\n');
            }
        }
    }

    src.push_str(&user_provided_imports);
    src.push_str("\n");

    let execute_param_name = if params_array.len() == 0 { "_" } else { "o" };
    src.push_str(&format!("pub fn execute({}: DataObject) -> DataObject {{\n", execute_param_name));

    let (param_extraction_code, function_call_args, user_fn_param_defs) =
        generate_rust_invoke_parts(&command_name, params_array.clone(), &rust_return_type);
    let return_packaging_code = generate_rust_return_packaging(&rust_return_type);

    src.push_str(&param_extraction_code);
    src.push_str(&format!("    let ax = {}({});\n", command_name, function_call_args));
    src.push_str("    let mut result_obj = DataObject::new();\n");
    src.push_str(&return_packaging_code);
    src.push_str("    result_obj\n"); src.push_str("}\n\n");

    src.push_str(&format!("pub fn {}({}) -> {} {{\n", command_name, user_fn_param_defs, rust_return_type));
    src.push_str(user_code); src.push_str("\n}\n");
    src
}

fn generate_rust_invoke_parts(_user_fn_name: &str, params: DataArray, _rust_return_type: &str)
    -> (String, String, String) {
    let mut param_extraction_code = String::new();
    let mut function_call_args = String::new();
    let mut user_fn_param_defs = String::new();

    for (index, param_value) in params.objects().iter().enumerate() {
        let param_obj = param_value.object();
        let name = param_obj.get_string("name");
        let meta_type = param_obj.get_string("type");
        let rust_type = map_meta_type_to_rust_type(&meta_type);
        let arg_var_name = format!("arg_{}", index);

        let getter_suffix = match rust_type.as_str() {
            "DataObject" => "object", "DataArray" => "array", "DataBytes" => "bytes",
            "Data" => "property", "bool" => "boolean", "i64" => "int",
            "f64" => "float", "String" => "string", _ => "property",
        };
        param_extraction_code.push_str(&format!(
            "    let {}: {} = o.get_{}(\"{}\");\n", arg_var_name, rust_type, getter_suffix, name));

        if index > 0 { function_call_args.push_str(", "); user_fn_param_defs.push_str(", "); }
        function_call_args.push_str(&arg_var_name);
        user_fn_param_defs.push_str(&format!("{}: {}", name, rust_type));
    }
    (param_extraction_code, function_call_args, user_fn_param_defs)
}

fn generate_rust_return_packaging(rust_return_type: &str) -> String {
    let mut s = String::new();
    if rust_return_type == "Data" {
        s.push_str("    result_obj.set_property(\"a\", ax);\n");
    } else {
        let putter_suffix = match rust_return_type {
            "String" => "string(\"a\", &ax)", "f64" => "float(\"a\", ax)",
            "i64" => "int(\"a\", ax)", "bool" => "boolean(\"a\", ax)",
            "DataObject" => "object(\"a\", ax)", "DataArray" => "array(\"a\", ax)",
            "DataBytes" => "bytes(\"a\", ax)",
            _ => { eprintln!("Warning: Unhandled Rust return type for packaging: {}", rust_return_type);
                   "property(\"a\", Data::from(ax))" }
        };
        s.push_str(&format!("    result_obj.put_{};\n", putter_suffix));
    }
    s
}

fn build_python_command_source(output_path: &Path, meta: DataObject, user_code: &str) {
    let command_name = meta.get_string("cmd");
    let python_output_file = output_path.join(format!("{}.py", command_name));

    let data_section = meta.get_object("data");
    let params_array = data_section.get_array("params");
    let imports = data_section.get_string("import").replace("\r\n", "\n").replace("\r", "\n");

    let mut user_fn_param_names = Vec::new();
    let mut execute_fn_arg_extraction = Vec::new();

    for param_value in params_array.objects() {
        let param_obj = param_value.object();
        let name = param_obj.get_string("name");
        user_fn_param_names.push(name.clone());
        execute_fn_arg_extraction.push(format!("args['{}']", name));
    }

    let user_fn_params_str = user_fn_param_names.join(", ");
    let execute_args_str = execute_fn_arg_extraction.join(", ");

    let mut generated_src = String::new();
    generated_src.push_str(&imports); generated_src.push_str("\n\n");
    generated_src.push_str(&format!("def execute(args):\n    return {}({})\n\n", command_name, execute_args_str));
    generated_src.push_str(&format!("def {}({}):\n", command_name, user_fn_params_str));

    for line in BufReader::new(user_code.as_bytes()).lines() {
        generated_src.push_str(&format!("    {}\n", line.expect("Failed to read line from Python user code")));
    }

    generated_src.push_str("\n\nif __name__ == \"__main__\":\n");
    generated_src.push_str("    import sys\n    import json\n");
    generated_src.push_str("    print(json.dumps(execute(json.loads(sys.argv[1]))))\n");

    if let Some(parent) = python_output_file.parent() {
        if !parent.exists() {
            create_dir_all(parent).expect("Failed to create directory for python command source");
        }
    }
    std::fs::write(&python_output_file, generated_src)
        .expect(&format!("Unable to write Python file: {:?}", python_output_file));
}

fn lookup_rust_api_data_type(meta_type: &str) -> &str {
    match meta_type {
        "FLAT" | "JSONObject" => "DataObject", "JSONArray" => "DataArray",
        "InputStream" => "DataBytes", "float" => "f64", "Integer" => "i64",
        "Boolean" => "bool", "Any" => "Data", "NULL" => "DNull",
        _ => "String",
    }
}

fn lookup_rust_api_ndata_method_suffix(meta_type: &str) -> &str {
    match meta_type {
        "FLAT" | "JSONObject" => "object", "JSONArray" => "array",
        "InputStream" => "bytes", "float" => "float", "Integer" => "int",
        "Boolean" => "boolean", "Any" => "property", "NULL" => "null",
        _ => "string",
    }
}
