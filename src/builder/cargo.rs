//! This file is dedicated to all interactions with Cargo.toml files,
//! including creating them and updating their dependencies.

use std::collections::HashMap;
use std::fs::create_dir_all;
use std::path::PathBuf;

use ndata::dataobject::DataObject;

use super::util::{get_project_top_level_path, read_lines_from_file, write_lines_to_file};

/// Creates or updates a Cargo.toml file for a given library.
/// It dynamically determines core dependency versions from the root Cargo.toml.
pub(crate) fn update_cargo_toml(cargo_toml_path: &PathBuf, cargo_config: &DataObject, lib_name: &str, default_package_name: &str, is_ffi: bool) -> bool {
    let mut file_was_created = false;
    if !cargo_toml_path.exists() {
        if default_package_name != "main_project" {
            println!("Cargo.toml not found at {:?} for sub-project '{}' (library {}), creating default.", cargo_toml_path, default_package_name, lib_name);

            // Dynamically get the core dependency lines from the root Cargo.toml.
            let (flowlang_dep_line, ndata_dep_line) = get_core_dependency_lines();

            let crate_types_str = if is_ffi {
                "[\"dylib\"]".to_string()
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
{}
{}
serde = {{ version = "1.0", features = ["derive"], optional = true }}
serde_json = {{ version = "1.0", optional = true }}

[features]
reload = []
default = []
"# , default_package_name, crate_types_str, flowlang_dep_line, ndata_dep_line);

            if let Some(parent_dir) = cargo_toml_path.parent() {
                create_dir_all(parent_dir).expect("Failed to create parent directory for new Cargo.toml");
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

/// Reads the root Cargo.toml to find the dependency lines for flowlang and ndata.
fn get_core_dependency_lines() -> (String, String) {
    let root_cargo_path = get_project_top_level_path().join("Cargo.toml");
    let fallback_flowlang = "flowlang = { version = \"0.3.29\" }".to_string();
    let fallback_ndata = "ndata = { version = \"0.3.16\" }".to_string();

    if !root_cargo_path.exists() {
        println!("WARNING: Root Cargo.toml not found. Falling back to default dependency versions.");
        return (fallback_flowlang, fallback_ndata);
    }

    let lines = read_lines_from_file(&root_cargo_path).unwrap_or_else(|_| {
        println!("WARNING: Could not read root Cargo.toml. Falling back to default dependency versions.");
        vec![]
    });

    let mut flowlang_line = None;
    let mut ndata_line = None;
    let mut in_deps_section = false;

    for line in lines {
        let trimmed_line = line.trim();
        if trimmed_line == "[dependencies]" {
            in_deps_section = true;
            continue;
        }
        if in_deps_section && trimmed_line.starts_with('[') {
            break; // Moved to the next section
        }
        if in_deps_section {
            if trimmed_line.starts_with("flowlang") {
                flowlang_line = Some(line.clone());
            } else if trimmed_line.starts_with("ndata") {
                ndata_line = Some(line.clone());
            }
        }
        if flowlang_line.is_some() && ndata_line.is_some() {
            break;
        }
    }
    (flowlang_line.unwrap_or(fallback_flowlang), ndata_line.unwrap_or(fallback_ndata))
}

/// Parses a `Cargo.toml`'s lines to find where a new entry in a section should be inserted.
pub(crate) fn find_section_insertion_line(
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
                let value_part = trimmed_line[eq_offset + 1..].trim().to_string();
                existing_items_map.insert(key, value_part);
            }
        }
    }
    section_start_idx.map_or(lines.len(), |idx| idx + 1)
}


/// Updates a section in a `Cargo.toml` (represented as a Vec of lines) with new items.
pub(crate) fn update_cargo_section_lines(
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

    for (key, value_obj) in new_items_config.objects() {
        let value_from_meta = value_obj.string();

        let new_line_content = if value_from_meta.trim().starts_with('{') {
            format!("{} = {}", key, value_from_meta)
        } else {
            let normalized_version = value_from_meta.trim().trim_matches('"');
            format!("{} = \"{}\"", key, normalized_version)
        };

        if let Some(existing_line_value) = section_items_map.get(&key) {
            if existing_line_value.contains("path") {
                continue;
            }

            let normalized_existing = existing_line_value.trim().trim_matches('"');
            let normalized_meta = value_from_meta.trim().trim_matches('"');

            if normalized_existing != normalized_meta {
                 println!(
                    "WARNING: {} '{}' in Cargo.toml for library \"{}\" (current value: {}) does not match config value ({}). Updating.",
                    item_type_name, key, lib_name, existing_line_value, value_from_meta
                );

                let mut updated = false;
                for line_idx in (0..cargo_lines.len()).rev() {
                    if cargo_lines[line_idx].trim().starts_with(&format!("{} =", key)) {
                        cargo_lines[line_idx] = new_line_content.clone();
                        section_modified = true;
                        updated = true;
                        break;
                    }
                }
                if !updated {
                    println!("WARNING: Could not find existing {} line for '{}' in Cargo.toml to update.", item_type_name, key);
                }
            }

        } else {
            println!("Adding new {} to Cargo.toml for library \"{}\": {}", item_type_name, lib_name, new_line_content);
            cargo_lines.insert(current_insertion_idx, new_line_content.clone());
            section_modified = true;
            current_insertion_idx += 1;
        }
    }
    (section_modified, current_insertion_idx)
}
