use std::collections::{HashMap, HashSet}; // Added HashSet
use std::fs::{create_dir_all, read_dir, read_to_string, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use ndata::dataarray::DataArray;
use ndata::dataobject::DataObject;
// For robust relative path calculation, the 'pathdiff' crate would be ideal.
// Since it's not specified as available, we'll do simpler relative path construction
// for the specific case needed, assuming a known structure.
// use pathdiff; // Example if it were available

use crate::DataStore;

const CMD_MOD_LINE: &str =
    "pub fn cmdinit(cmds: &mut Vec<(String, flowlang::rustcmd::Transform, String)>) {";

// Helper to get the assumed project top-level directory.
fn get_project_top_level_path() -> PathBuf {
    std::env::current_dir().expect("Failed to get current directory, which is assumed to be project root.")
}


// --- Public API Functions ---

pub fn build_all() -> bool {
    let mut overall_build_occurred = false;
    let lib_entries = read_dir("data")
        .expect("Failed to read 'data' directory. This directory is essential for the build process.");

    for dir_entry_result in lib_entries {
        let dir_entry = dir_entry_result
            .expect("Error reading a directory entry in 'data'. Check directory permissions and integrity.");
        let lib_name = dir_entry
            .file_name()
            .into_string()
            .expect("Library name is not valid UTF-8. All library names in 'data' must be valid UTF-8 strings.");

        if build_lib(lib_name) {
            overall_build_occurred = true;
        }
    }
    overall_build_occurred
}

pub fn build_lib(lib_name: String) -> bool {
    let mut build_actions_performed = false;
    let store = DataStore::new(); // Create store once for the library build

    let project_top_level_path = get_project_top_level_path();

    let lib_metadata = store.lib_info(&lib_name);

    let lib_config_root_field: String;
    if lib_metadata.has("root") {
        let value_from_meta = lib_metadata.get_string("root"); // Safe now
        if value_from_meta.is_empty() { // Handles case where "root": ""
            lib_config_root_field = "cmd".to_string();
        } else {
            lib_config_root_field = value_from_meta;
        }
    } else { // "root" key does not exist
        lib_config_root_field = "cmd".to_string();
    }


    let library_build_base_path = if lib_config_root_field == "." {
        project_top_level_path.clone()
    } else {
        project_top_level_path.join(&lib_config_root_field)
    };

    let library_data_path_for_build_fn = store.get_lib_root(&lib_name);


    if !store.exists(&lib_name, "controls") {
        println!("No controls definition found in library: {}", &lib_name);
    } else {
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
                // Process commands for this control
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
                            &library_data_path_for_build_fn,
                        ) {
                            build_actions_performed = true;
                        }
                    }
                }
            }
        }
    }

    // FIXME: This line was `if true { b = true; }` in the original code.
    if true {
        build_actions_performed = true;
    }

    if build_actions_performed {
        let cargo_toml_path = library_build_base_path.join("Cargo.toml");
        let package_name_for_default_cargo = if lib_config_root_field == "." {
            "main_project".to_string()
        } else {
            lib_config_root_field.clone()
        };

        // cargo_config_updates will be used both for creating a default Cargo.toml (for crate_types)
        // and for updating an existing one.
        let cargo_config_updates = if lib_metadata.has("cargo") {
            lib_metadata.get_object("cargo")
        } else {
            DataObject::new() // Empty config if "cargo" section is missing
        };

        if update_cargo_toml(&cargo_toml_path, &cargo_config_updates, &lib_name, &package_name_for_default_cargo) {
            // build_actions_performed is already true if file was created or modified
        }
    }

    build_actions_performed
}

// REVERTED SIGNATURE to match original public API
pub fn build(
    lib_name: &str,
    control_name: &str,
    command_name: &str,
    _library_data_path_arg: &Path, // Prefixed with underscore to silence unused warning
) -> bool {
    let mut artifact_changed = false;
    let store = DataStore::new();

    // --- Derive necessary paths and configurations internally ---
    let project_top_level_path = get_project_top_level_path();
    let top_level_project_src_path = project_top_level_path.join("src");

    let lib_metadata = store.lib_info(lib_name);

    let lib_config_root_field: String;
    if lib_metadata.has("root") {
        let value_from_meta = lib_metadata.get_string("root"); // Safe now
        if value_from_meta.is_empty() { // Handles case where "root": ""
            lib_config_root_field = "cmd".to_string();
        } else {
            lib_config_root_field = value_from_meta;
        }
    } else { // "root" key does not exist
        lib_config_root_field = "cmd".to_string();
    }


    let library_build_base_path = if lib_config_root_field == "." {
        project_top_level_path.clone()
    } else {
        project_top_level_path.join(&lib_config_root_field)
    };
    let actual_library_build_src_path = library_build_base_path.join("src");
    // --- End of internal derivation ---

    let command_id = store.lookup_cmd_id(lib_name, control_name, command_name);

    if store.exists(lib_name, &command_id) {
        let command_metadata = store.get_data(lib_name, &command_id);
        let data_section = command_metadata.get_object("data");
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
                    &actual_library_build_src_path,
                    &top_level_project_src_path,
                    &lib_config_root_field,
                    lib_name,
                    control_name,
                    command_name,
                    &rust_file_id,
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
                // FIXME - JS/Java/Flow
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

pub fn rebuild_rust_api() {
    let store = DataStore::new();
    let project_top_level_path = get_project_top_level_path();
    let api_file_path = project_top_level_path.join("cmd").join("src").join("api.rs"); // Assuming api.rs is for "cmd" sub-project

    let mut api_struct_init_str = "pub const fn new() -> api {\n  api {\n".to_string();
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
            api_struct_init_str.push_str(&format!("    {}: {} {{\n", lib_name, lib_name));
            api_struct_def_str.push_str(&format!("  pub {}: {},\n", lib_name, lib_name));
            control_struct_defs_str.push_str(&format!("pub struct {} {{\n", lib_name));

            let controls_data = store.get_data(&lib_name, "controls");
            let list = controls_data.get_object("data").get_array("list");

            for control_val in list.objects() {
                let control = control_val.object();
                let ctl_name = control.get_string("name");
                let ctl_id = control.get_string("id");

                if store.exists(&lib_name, &ctl_id) {
                    let struct_name = format!("{}_{}", lib_name, ctl_name);
                    api_struct_init_str.push_str(&format!("      {}: {} {{}},\n", ctl_name, struct_name));
                    control_struct_defs_str.push_str(&format!("  pub {}: {},\n", ctl_name, struct_name));
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
                                let cmd_id_in_control = command.get_string("id");

                                if store.exists(&lib_name, &cmd_id_in_control) {
                                    let meta_for_cmd_type = store.get_data(&lib_name, &cmd_id_in_control);
                                    let data_for_cmd_type = meta_for_cmd_type.get_object("data");
                                    let typ = data_for_cmd_type.get_string("type");

                                    if typ == "rust" {
                                        let rust_meta_file_id = data_for_cmd_type.get_string("rust");
                                        impl_blocks_str.push_str(&format!("  pub fn {} (&self", cmd_name));
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
                                                "    d.{}_{}(\"{}\", {}{});\n",
                                                method_prefix, ntype, pname, q, pname
                                            ));
                                        }
                                        impl_blocks_str.push_str(&params_str_for_fn_def);
                                        let rtype_str = rust_cmd_actual_meta.get_string("returntype");
                                        let ntype_ret = lookup_rust_api_ndata_method_suffix(&rtype_str);
                                        let rtype_rust = lookup_rust_api_data_type(&rtype_str);
                                        impl_blocks_str.push_str(&format!(") -> {} {{\n", rtype_rust));

                                        if params_array.len() > 0 {
                                            impl_blocks_str.push_str("    let mut d = DataObject::new();\n");
                                        } else {
                                            impl_blocks_str.push_str("    let d = DataObject::new();\n");
                                        }
                                        impl_blocks_str.push_str(&params_setup_str_for_body);
                                        impl_blocks_str.push_str(&format!(
                                            "    RustCmd::new(\"{}\").execute(d).expect(\"Rust command execution failed\").get_{}(\"a\")\n  }}\n",
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
            api_struct_init_str.push_str("    },\n");
            control_struct_defs_str.push_str("}\n");
        }
    }
    api_struct_init_str.push_str("  }\n}\n");
    api_struct_def_str.push_str("}");

    let use_statements = r#"use ndata::dataobject::DataObject;
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

    if let Some(parent_dir) = api_file_path.parent() {
        if !parent_dir.exists() {
            create_dir_all(parent_dir).expect(&format!("Failed to create directory for api.rs: {:?}", parent_dir));
        }
    }
    std::fs::write(&api_file_path, final_api_code)
        .expect(&format!("Unable to write API file to {:?}", api_file_path));
}


// --- Helper Functions for Cargo.toml manipulation ---
// Added default_package_name parameter
fn update_cargo_toml(cargo_toml_path: &PathBuf, cargo_config: &DataObject, lib_name: &str, default_package_name: &str) -> bool {
    let mut file_created_or_modified = false;
    if !cargo_toml_path.exists() {
        if default_package_name != "main_project" {
            println!("Cargo.toml not found at {:?} for sub-project '{}' (library {}), creating default.", cargo_toml_path, default_package_name, lib_name);

            let crate_types_str = if cargo_config.has("crate_types") {
                let crate_types_da = cargo_config.get_array("crate_types");
                let types: Vec<String> = crate_types_da.objects().iter()
                    .map(|val| val.string())
                    .collect();
                if !types.is_empty() {
                    format!("[{}]", types.iter().map(|s| format!("\"{}\"", s)).collect::<Vec<String>>().join(", "))
                } else {
                    "[\"rlib\", \"dylib\"]".to_string() // Default if array is empty
                }
            } else {
                "[\"rlib\", \"dylib\"]".to_string() // Default if field doesn't exist
            };

            let default_content = format!(
r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = {}

[lints.rust]
non_camel_case_types = "allow"

[features]
serde_support = ["serde","serde_json","flowlang/serde_support","ndata/serde_support"]
reload = []

[dependencies]
flowlang = "0.3.15"
ndata = "0.3.10"
serde = {{ version = "1.0.155", features = ["derive"], optional = true }}
serde_json = {{ version = "1.0.94", optional = true }}
"#, default_package_name, crate_types_str);
            if let Some(parent_dir) = cargo_toml_path.parent() {
                if !parent_dir.exists() {
                    create_dir_all(parent_dir).expect("Failed to create parent directory for new Cargo.toml");
                }
            }
            std::fs::write(&cargo_toml_path, default_content)
                .expect(&format!("Failed to write default Cargo.toml to {:?}", cargo_toml_path));
            file_created_or_modified = true;
        } else {
             println!("Cargo.toml not found at {:?} for main project (library {}), skipping creation of default.", cargo_toml_path, lib_name);
            return false;
        }
    }

    let file = File::open(&cargo_toml_path)
        .expect(&format!("Failed to open Cargo.toml at {:?}", cargo_toml_path));
    let mut lines: Vec<String> = BufReader::new(file).lines().map(|l| l.expect("Failed to read line from Cargo.toml")).collect();

    let mut features_map = HashMap::new();
    let mut dependencies_map = HashMap::new();

    let features_insertion_line = find_section_insertion_line(&lines, "[features]", &mut features_map);
    let dependencies_insertion_line = find_section_insertion_line(&lines, "[dependencies]", &mut dependencies_map);

    let mut config_caused_modification = false;

    if cargo_config.has("features") {
        let new_features = cargo_config.get_object("features");
        if new_features.clone().keys().len() > 0 {
            let (section_modified, _new_insertion_idx) = update_cargo_section_lines(
                &mut lines,
                &new_features,
                &mut features_map,
                features_insertion_line,
                "Feature",
                lib_name,
            );
            if section_modified {
                config_caused_modification = true;
            }
        }
    }

    if cargo_config.has("dependencies") {
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
        let mut outfile = File::create(&cargo_toml_path)
            .expect(&format!("Failed to create/truncate Cargo.toml at {:?}", cargo_toml_path));
        for line in lines {
            writeln!(outfile, "{}", line)
                .expect(&format!("Failed to write to Cargo.toml at {:?}", cargo_toml_path));
        }
        file_created_or_modified = true;
    }
    file_created_or_modified
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

// Added lib_name parameter
fn update_cargo_section_lines(
    cargo_lines: &mut Vec<String>,
    new_items_config: &DataObject,
    section_items_map: &mut HashMap<String, String>,
    mut current_insertion_idx: usize,
    item_type_name: &str,
    lib_name: &str, // New parameter
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


// --- Helper Functions for mod.rs and other file manipulations ---
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
    mod_line_to_add: &str,
    cmd_init_item_to_add: Option<&str>,
    _is_project_level_cmdinit: bool, // Prefixed with underscore
) {
    let file_existed_initially = mod_file_path.exists();
    let mut lines = if file_existed_initially {
        read_lines_from_file(mod_file_path)
            .expect(&format!("Failed to read mod file: {:?}", mod_file_path))
    } else {
        let mut initial_lines = Vec::new();
        if cmd_init_item_to_add.is_some() {
            initial_lines.push(String::new());
            initial_lines.push(CMD_MOD_LINE.to_string());
            initial_lines.push("}".to_string());
        } else if !mod_line_to_add.trim().is_empty() {
            initial_lines.push(mod_line_to_add.to_string());
        }
        initial_lines
    };

    let mut modified = false;
    if !file_existed_initially && !lines.is_empty() {
        modified = true;
    }


    let mod_line_trimmed = mod_line_to_add.trim();
    let main_decl_line = mod_line_to_add.lines().last().unwrap_or("").trim();

    let mut declaration_exists = false;
    if mod_line_trimmed.starts_with("#[path") {
        if lines.join("\n").contains(mod_line_trimmed) {
            declaration_exists = true;
        }
    } else {
        if lines.iter().any(|l| l.trim() == mod_line_trimmed) {
            declaration_exists = true;
        }
    }

    if !declaration_exists {
        if mod_line_trimmed.starts_with("#[path") {
            let simple_pub_mod_line = format!("pub mod {};", main_decl_line.split_whitespace().last().unwrap_or(""));
            if let Some(idx) = find_line_index_in_slice(&lines, &simple_pub_mod_line) {
                lines.remove(idx);
                lines.insert(idx, mod_line_to_add.to_string());
            } else {
                let insert_at = lines.iter().rposition(|l| l.trim().starts_with("#[path") || l.trim().starts_with("pub mod ")).map_or(0, |i| i + 1);
                lines.insert(insert_at, mod_line_to_add.to_string());
            }
        } else {
            let insert_at = if mod_line_trimmed.starts_with("use ") {
                lines.iter().rposition(|l| l.trim().starts_with("use ")).map_or(0, |i| i + 1)
            } else {
                 lines.iter().rposition(|l| l.trim().starts_with("pub mod ")).map_or(0, |i| i + 1)
            };
            lines.insert(insert_at, mod_line_to_add.to_string());
        }
        modified = true;
    }


    if let Some(item_to_add) = cmd_init_item_to_add {
        let item_to_add_trimmed = item_to_add.trim();
        match find_line_index_in_slice(&lines, CMD_MOD_LINE) {
            Some(cmd_init_header_idx) => {
                let mut item_exists_in_cmdinit = false;
                for i in cmd_init_header_idx + 1..lines.len() {
                    let current_line_trimmed = lines[i].trim();
                    if current_line_trimmed == "}" { break; }
                    if current_line_trimmed == item_to_add_trimmed {
                        item_exists_in_cmdinit = true;
                        break;
                    }
                }
                if !item_exists_in_cmdinit {
                    lines.insert(cmd_init_header_idx + 1, item_to_add.to_string());
                    modified = true;
                }
            }
            None => {
                lines.push(String::new());
                lines.push(CMD_MOD_LINE.to_string());
                lines.push(item_to_add.to_string());
                lines.push("}".to_string());
                modified = true;
            }
        }
    }

    if modified {
        write_lines_to_file(mod_file_path, &lines)
            .expect(&format!("Unable to write mod file: {:?}", mod_file_path));
    }
}

fn build_mod_files_for_rust_command(
    command_output_path: &Path,
    library_actual_src_path: &Path,
    top_level_project_src_path: &Path,
    lib_config_root_field: &str,
    lib_name: &str,
    control_name: &str,
    command_name: &str,
    rust_cmd_meta_id: &str,
) {
    // 1. Update control's mod.rs
    let ctl_mod_file = command_output_path.join("mod.rs");
    let mod_line_for_cmd_in_ctl = format!("pub mod {};", command_name);
    let cmd_push_line_for_cmd_in_ctl = format!(
        "    cmds.push((\"{}\".to_string(), {}::execute, \"\".to_string()));",
        rust_cmd_meta_id, command_name
    );
    update_mod_file_content(&ctl_mod_file, &mod_line_for_cmd_in_ctl, Some(&cmd_push_line_for_cmd_in_ctl), false);

    // 2. Update library's mod.rs
    let lib_mod_path = command_output_path.parent()
        .expect("Command output path should have a parent (library module level)");
    let lib_mod_file = lib_mod_path.join("mod.rs");
    let mod_line_for_ctl_in_lib = format!("pub mod {};", control_name);
    let cmd_init_call_for_ctl_in_lib = format!("    {}::cmdinit(cmds);", control_name);
    update_mod_file_content(&lib_mod_file, &mod_line_for_ctl_in_lib, Some(&cmd_init_call_for_ctl_in_lib), false);

    // 3. Update project-level cmdinit.rs
    let project_cmd_init_rs_path = top_level_project_src_path.join("cmdinit.rs");
    let use_line_for_main_cmdinit: String;
    let call_line_for_main_cmdinit: String;

    if lib_config_root_field == "." {
        use_line_for_main_cmdinit = format!("use crate::{};", lib_name);
        call_line_for_main_cmdinit = format!("    {}::cmdinit(cmds);", lib_name);
    } else {
        let sub_project_crate_name = lib_config_root_field;
        use_line_for_main_cmdinit = format!("use {};", sub_project_crate_name);
        call_line_for_main_cmdinit = format!("    {}::cmdinit(cmds);", sub_project_crate_name);
    }

    let line_to_remove_1 = format!("    cmds.push((\"{}\".to_string(), {}::{}::{}::execute, \"\".to_string()));", rust_cmd_meta_id, lib_name, control_name, command_name);
    let line_to_remove_2 = "    cmds.clear();".to_string();
    if project_cmd_init_rs_path.exists() {
        let mut cmd_init_lines = read_lines_from_file(&project_cmd_init_rs_path).expect("Failed to read project cmdinit.rs");
        let initial_len = cmd_init_lines.len();
        cmd_init_lines.retain(|line| line.trim() != line_to_remove_1.trim());
        cmd_init_lines.retain(|line| line.trim() != line_to_remove_2.trim());
        if cmd_init_lines.len() != initial_len {
            write_lines_to_file(&project_cmd_init_rs_path, &cmd_init_lines).expect("Failed to write project cmdinit.rs after removals");
        }
    }
    update_mod_file_content(&project_cmd_init_rs_path, &use_line_for_main_cmdinit, Some(&call_line_for_main_cmdinit), true);

    // 4. Update the sub-project's structure (lib.rs/main.rs and its own cmdinit.rs)
    if lib_config_root_field != "." {
        let sub_project_src_path = library_actual_src_path;

        // 4a. Ensure and Populate sub-project's cmdinit.rs (e.g. /newbound/cmd/src/cmdinit.rs)
        let sub_project_cmdinit_file_path = sub_project_src_path.join("cmdinit.rs");
        if !sub_project_cmdinit_file_path.exists() {
            println!("Creating default cmdinit.rs for sub-project at {:?}", sub_project_cmdinit_file_path);
            let default_cmdinit_content = "pub fn cmdinit(cmds: &mut Vec<(String, flowlang::rustcmd::Transform, String)>) {\n\
}\n";
            if let Some(parent_dir) = sub_project_cmdinit_file_path.parent() {
                if !parent_dir.exists() {
                    create_dir_all(parent_dir).expect("Failed to create src dir for sub-project cmdinit.rs");
                }
            }
            std::fs::write(&sub_project_cmdinit_file_path, default_cmdinit_content)
                .expect("Failed to write default cmdinit.rs for sub-project");
        }
        // Populate sub-project's cmdinit.rs with: use crate::lib_name; lib_name::cmdinit(cmds);
        // Note: `lib_name` is the specific library (e.g., "storage") being processed in the current iteration.
        // The sub-project's cmdinit.rs should call the cmdinit of each library *within* that sub-project.
        let use_line_for_sub_project_cmdinit = format!("use crate::{};", lib_name);
        let call_line_for_sub_project_cmdinit = format!("    {}::cmdinit(cmds);", lib_name);
        update_mod_file_content(&sub_project_cmdinit_file_path, &use_line_for_sub_project_cmdinit, Some(&call_line_for_sub_project_cmdinit), true);

        // 4b. Update sub-project's lib.rs/main.rs
        for entry_point_filename in ["lib.rs", "main.rs"] {
            let sub_project_entry_point_path = sub_project_src_path.join(entry_point_filename);

            if !sub_project_entry_point_path.exists() {
                let default_content: Option<String> = if entry_point_filename == "lib.rs" {
                    println!("Creating default lib.rs for sub-project at {:?}", sub_project_entry_point_path);
                    Some(r#"// `pub mod library_name;` lines will be added by the build script.

pub use cmdinit::cmdinit;
mod cmdinit;

use flowlang::rustcmd::*;
use ndata::NDataConfig;

mod api;
pub static API : crate::api::api = crate::api::new();

#[derive(Debug)]
pub struct Initializer {
  pub data_ref: (&'static str, NDataConfig),
  pub cmds: Vec<(String, Transform, String)>,
}

#[no_mangle]
pub fn mirror(state: &mut Initializer) {
  #[cfg(feature = "reload")]
  flowlang::mirror(state.data_ref);
  cmdinit(&mut state.cmds);
  #[cfg(feature = "reload")]
  for q in &state.cmds { RustCmd::add(q.0.to_owned(), q.1, q.2.to_owned()); }
}"#.to_string())
                } else if entry_point_filename == "main.rs" {
                     println!("Creating default main.rs for sub-project at {:?}", sub_project_entry_point_path);
                    Some(r#"// `pub mod library_name;` lines will be added by the build script.

pub use cmdinit::cmdinit;
mod cmdinit;

use std::env;
use flowlang::appserver::*;
use flowlang::rustcmd::*;

mod api;
pub static API : crate::api::api = crate::api::new();

fn main() {
  flowlang::init("data");
  init_cmds();

  env::set_var("RUST_BACKTRACE", "1");
  {
    run();
  }
}

fn init_cmds(){
  let mut v = Vec::new();
  cmdinit(&mut v);
  for q in &v { RustCmd::add(q.0.to_owned(), q.1, q.2.to_owned()); }
}"#.to_string())
                } else {
                    None
                };

                if let Some(content) = default_content {
                    if let Some(parent_dir) = sub_project_entry_point_path.parent(){
                        if !parent_dir.exists(){
                             create_dir_all(parent_dir).expect("Failed to create src dir for sub-project entry point");
                        }
                    }
                    std::fs::write(&sub_project_entry_point_path, content)
                        .expect(&format!("Failed to write default {} to {:?}", entry_point_filename, sub_project_entry_point_path));
                }
            }

            if sub_project_entry_point_path.exists() {
                let mod_decl_for_sub_project_lib = format!("pub mod {};", lib_name);
                update_mod_file_content(&sub_project_entry_point_path, &mod_decl_for_sub_project_lib, None, false);

                let cmdinit_mod_decl = "mod cmdinit;";
                let cmdinit_pub_use = "pub use cmdinit::cmdinit;";
                update_mod_file_content(&sub_project_entry_point_path, cmdinit_mod_decl, None, false);
                update_mod_file_content(&sub_project_entry_point_path, cmdinit_pub_use, None, false);
            }
        }
    }

    // 5. Update the MAIN project's lib.rs/main.rs
    if lib_config_root_field == "." {
        let mod_decl_for_main_project_entry = format!("pub mod {};", lib_name);
        for entry_point_filename in ["lib.rs", "main.rs"] {
            let main_project_entry_point_path = top_level_project_src_path.join(entry_point_filename);
            if main_project_entry_point_path.exists() {
                update_mod_file_content(&main_project_entry_point_path, &mod_decl_for_main_project_entry, None, true);
            } else {
                // FIXME - else what? (If main project's lib.rs/main.rs doesn't exist)
            }
        }
    }
}


// --- Helper Functions for Rust Command Source Generation ---
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
        let old_src = read_to_string(&rust_output_file)
            .expect(&format!("Failed to read existing Rust file: {:?}", rust_output_file));
        // FIXME - what if compile files and we try again?
        if old_src == generated_src {
            needs_write = false;
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

    // Determine ndata types needed by the generated execute() wrapper
    let mut wrapper_ndata_types_needed = HashSet::new();
    wrapper_ndata_types_needed.insert("DataObject".to_string()); // Always for `o` and `result_obj`

    for param_value in params_array.objects() {
        let param_obj = param_value.object();
        let meta_type = param_obj.get_string("type");
        let rust_type = map_meta_type_to_rust_type(&meta_type);
        if ["DataObject", "DataArray", "DataBytes", "Data"].contains(&rust_type.as_str()) {
            wrapper_ndata_types_needed.insert(rust_type);
        }
    }
    let rust_return_type = map_meta_type_to_rust_type(&returntype_meta_str);
    if ["DataObject", "DataArray", "DataBytes", "Data"].contains(&rust_return_type.as_str()) {
        wrapper_ndata_types_needed.insert(rust_return_type.clone());
    }

    // Add specific ndata use statements only if needed by wrapper and not obviously covered by user
    let ndata_types_to_import = [
        ("DataObject", "ndata::dataobject"),
        ("DataArray", "ndata::dataarray"),
        ("DataBytes", "ndata::databytes"),
        ("Data", "ndata::data"),
    ];

    for (type_name_str, module_path_str) in ndata_types_to_import.iter() {
        if wrapper_ndata_types_needed.contains(*type_name_str) {
            let full_type_path = format!("{}::{}", module_path_str, type_name_str);
            let module_wildcard = format!("{}::*", module_path_str);
            let crate_wildcard = "use ndata::*;";

            let is_likely_covered_by_user = user_provided_imports.contains(&full_type_path) ||
                                            user_provided_imports.contains(&module_wildcard) ||
                                            user_provided_imports.contains(crate_wildcard);

            if !is_likely_covered_by_user {
                src.push_str(&format!("use {};\n", full_type_path));
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
    src.push_str(&format!("  let ax = {}({});\n", command_name, function_call_args));
    src.push_str("  let mut result_obj = DataObject::new();\n");
    src.push_str(&return_packaging_code);
    src.push_str("  result_obj\n"); src.push_str("}\n\n");

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
            "  let {}: {} = o.get_{}(\"{}\");\n", arg_var_name, rust_type, getter_suffix, name));

        if index > 0 { function_call_args.push_str(", "); user_fn_param_defs.push_str(", "); }
        function_call_args.push_str(&arg_var_name);
        user_fn_param_defs.push_str(&format!("{}: {}", name, rust_type));
    }
    (param_extraction_code, function_call_args, user_fn_param_defs)
}

fn generate_rust_return_packaging(rust_return_type: &str) -> String {
    let mut s = String::new();
    if rust_return_type == "Data" {
        s.push_str("  result_obj.set_property(\"a\", ax);\n");
    } else {
        let putter_suffix = match rust_return_type {
            "String" => "string(\"a\", &ax)", "f64" => "float(\"a\", ax)",
            "i64" => "int(\"a\", ax)", "bool" => "boolean(\"a\", ax)",
            "DataObject" => "object(\"a\", ax)", "DataArray" => "array(\"a\", ax)",
            "DataBytes" => "bytes(\"a\", ax)",
            _ => { eprintln!("Warning: Unhandled Rust return type for packaging: {}", rust_return_type);
                   "property(\"a\", Data::from(ax))" }
        };
        s.push_str(&format!("  result_obj.put_{};\n", putter_suffix));
    }
    s
}

// --- Helper Functions for Python Command Source Generation ---
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
    generated_src.push_str(&format!("def execute(args):\n  return {}({})\n\n", command_name, execute_args_str));
    generated_src.push_str(&format!("def {}({}):\n", command_name, user_fn_params_str));

    for line in BufReader::new(user_code.as_bytes()).lines() {
        generated_src.push_str(&format!("  {}\n", line.expect("Failed to read line from Python user code")));
    }

    generated_src.push_str("\n\nif __name__ == \"__main__\":\n");
    generated_src.push_str("  import sys\n  import json\n");
    generated_src.push_str("  print(json.dumps(execute(json.loads(sys.argv[1]))))\n");

    if let Some(parent) = python_output_file.parent() {
        if !parent.exists() {
            create_dir_all(parent).expect("Failed to create directory for python command source");
        }
    }
    std::fs::write(&python_output_file, generated_src)
        .expect(&format!("Unable to write Python file: {:?}", python_output_file));
}

// --- Helper functions for rebuild_rust_api ---
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
