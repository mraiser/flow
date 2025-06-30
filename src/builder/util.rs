//! This file contains shared helper functions used across the builder module,
//! such as file I/O operations, path manipulation, and various lookups.

use std::fs::{create_dir_all, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{PathBuf};

use ndata::dataobject::DataObject;

/// Helper to get the assumed project top-level directory.
pub(crate) fn get_project_top_level_path() -> PathBuf {
    std::env::current_dir().expect("Failed to get current directory, which is assumed to be project root.")
}

/// Extracts the crate's root directory name and FFI status from its metadata.
/// The root defaults to "cmd" if not specified.
pub(crate) fn get_crate_info(lib_metadata: &DataObject) -> (String, bool) {
    let root = if lib_metadata.has("root") {
        let value = lib_metadata.get_string("root");
        if value.is_empty() { "cmd".to_string() } else { value }
    } else {
        "cmd".to_string()
    };

    let is_ffi = if lib_metadata.has("cargo") {
        let cargo_obj = lib_metadata.get_object("cargo");
        // THIS IS THE FIX:
        // We must check if the 'ffi' key exists before trying to get it as a boolean.
        // The get_boolean function panics if the key is not found.
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

/// Reads all lines from a file into a Vec<String>.
pub(crate) fn read_lines_from_file(path: &PathBuf) -> Result<Vec<String>, std::io::Error> {
    let file = File::open(path)?;
    BufReader::new(file).lines().collect()
}

/// Writes a slice of strings to a file, with each string as a new line.
/// Creates parent directories if they don't exist.
pub(crate) fn write_lines_to_file(path: &PathBuf, lines: &[String]) -> Result<(), std::io::Error> {
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

/// Finds the line number of a specific marker string in a slice of strings.
pub(crate) fn find_line_index_in_slice(lines: &[String], marker: &str) -> Option<usize> {
    lines.iter().position(|line| line.trim() == marker.trim())
}

/// Idempotently adds a `use` statement and/or a line of code to a `mod.rs` file.
/// It checks if the lines already exist before adding them.
pub(crate) fn update_mod_file_content(
    mod_file_path: &PathBuf,
    line_to_add: &str,
    use_line_to_add: Option<&str>,
) {
    let mut lines = if mod_file_path.exists() {
        read_lines_from_file(mod_file_path)
            .expect(&format!("Failed to read mod file: {:?}", mod_file_path))
    } else {
        // If the file doesn't exist, we can't update it.
        // It should have been created by a scaffolding function first.
        return;
    };

    let original_content = lines.join("\n");
    let mut modified = false;

    // Add 'use' statement if provided and not already present.
    if let Some(use_line) = use_line_to_add {
        if !use_line.is_empty() && !original_content.contains(use_line) {
             // Insert after the last 'use' statement, or at the top.
             let insert_at = lines.iter().rposition(|l| l.trim().starts_with("use ")).map_or(0, |i| i + 1);
             lines.insert(insert_at, use_line.to_string());
             modified = true;
        }
    }

    // Add the main line of code if not already present.
    if !line_to_add.is_empty() && !original_content.contains(line_to_add) {
        if line_to_add.starts_with("pub mod") {
            // 'pub mod' statements usually go after 'use' statements.
            let insert_at = lines.iter().rposition(|l| l.trim().starts_with("use ")).map_or(0, |i| i + 1);
            lines.insert(insert_at, line_to_add.to_string());
            modified = true;
        } else {
            // For other lines, assume they go inside the cmdinit function body.
            if let Some(idx) = find_line_index_in_slice(&lines, "}") {
                 lines.insert(idx, format!("    {}", line_to_add));
                 modified = true;
            } else {
                 // Fallback if no closing brace is found (e.g., in a malformed file).
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


/// Looks up the Rust data type string corresponding to a flowlang metadata type.
pub(crate) fn lookup_rust_api_data_type(meta_type: &str) -> &str {
    match meta_type {
        "FLAT" | "JSONObject" => "DataObject", "JSONArray" => "DataArray",
        "InputStream" => "DataBytes", "float" => "f64", "Integer" => "i64",
        "Boolean" => "bool", "Any" => "Data", "NULL" => "DNull",
        _ => "String",
    }
}

/// Looks up the ndata getter/setter method suffix for a flowlang metadata type.
pub(crate) fn lookup_rust_api_ndata_method_suffix(meta_type: &str) -> &str {
    match meta_type {
        "FLAT" | "JSONObject" => "object", "JSONArray" => "array",
        "InputStream" => "bytes", "float" => "float", "Integer" => "int",
        "Boolean" => "boolean", "Any" => "property", "NULL" => "null",
        _ => "string",
    }
}
