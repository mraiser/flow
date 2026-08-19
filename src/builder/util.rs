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
/// The root defaults to "cmd" if not specified. Delegates to the shared
/// parser in `datastore` so the builder and the hotswap loader can never
/// disagree about what is FFI-rooted.
pub(crate) fn get_crate_info(lib_metadata: &DataObject) -> (String, bool) {
    crate::datastore::crate_info_from_meta(lib_metadata)
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
/// It checks if the lines already exist before adding them. Returns whether the
/// file was modified — a new command's only source change can be its mod-file
/// wiring (the generated command file may be byte-identical to a stale one), so
/// callers must count this toward their "did anything change" verdict or the
/// compile pipeline skips the build (and the hot-reload) entirely.
pub(crate) fn update_mod_file_content(
    mod_file_path: &PathBuf,
    line_to_add: &str,
    use_line_to_add: Option<&str>,
) -> bool {
    let mut lines = if mod_file_path.exists() {
        read_lines_from_file(mod_file_path)
            .expect(&format!("Failed to read mod file: {:?}", mod_file_path))
    } else {
        // If the file doesn't exist, we can't update it.
        // It should have been created by a scaffolding function first.
        return false;
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
    modified
}

/// Idempotently removes a line of code (and optionally a `use` statement) from
/// a `mod.rs`-style file. The inverse of `update_mod_file_content`, matched on
/// trimmed content so indentation (e.g. cmdinit body lines) doesn't matter.
pub(crate) fn remove_mod_file_content(
    mod_file_path: &PathBuf,
    line_to_remove: &str,
    use_line_to_remove: Option<&str>,
) {
    if !mod_file_path.exists() {
        return;
    }
    let lines = match read_lines_from_file(mod_file_path) {
        Ok(l) => l,
        Err(_) => return,
    };
    let original_len = lines.len();
    let kept: Vec<String> = lines
        .into_iter()
        .filter(|line| {
            let trimmed = line.trim();
            if !line_to_remove.is_empty() && trimmed == line_to_remove.trim() {
                return false;
            }
            if let Some(use_line) = use_line_to_remove {
                if !use_line.is_empty() && trimmed == use_line.trim() {
                    return false;
                }
            }
            true
        })
        .collect();
    if kept.len() != original_len {
        write_lines_to_file(mod_file_path, &kept)
            .expect(&format!("Unable to write mod file: {:?}", mod_file_path));
    }
}


/// Looks up the Rust data type string corresponding to a flowlang metadata type.
pub(crate) fn lookup_rust_api_data_type(meta_type: &str) -> &str {
    match meta_type {
        "FLAT" | "JSONObject" => "DataObject", "JSONArray" => "DataArray",
        "InputStream" => "DataBytes", "Float" => "f64", "Integer" => "i64",
        "Boolean" => "bool", "Any" => "Data", "NULL" => "DNull",
        _ => "String",
    }
}

/// Looks up the ndata getter/setter method suffix for a flowlang metadata type.
pub(crate) fn lookup_rust_api_ndata_method_suffix(meta_type: &str) -> &str {
    match meta_type {
        "FLAT" | "JSONObject" => "object", "JSONArray" => "array",
        "InputStream" => "bytes", "Float" => "float", "Integer" => "int",
        "Boolean" => "boolean", "Any" => "property", "NULL" => "null",
        _ => "string",
    }
}

#[cfg(test)]
mod tests {
    use super::update_mod_file_content;

    /// A new command's only source change can be its mod-file wiring, so the
    /// modified flag must be true on a real insertion and false on a no-op —
    /// compile's changed verdict (and the FFI hot-reload behind it) hangs on it.
    #[test]
    fn mod_file_updates_report_modification() {
        let dir = std::env::temp_dir().join(format!("util_mod_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("mod.rs");
        std::fs::write(&path, "pub fn cmdinit(cmds: &mut Vec<(String, Transform, String)>) {\n}\n").unwrap();

        let push_line = "cmds.push((\"id1\".to_string(), marker::execute, \"\".to_string()));";
        assert!(update_mod_file_content(&path, "pub mod marker;", None));
        assert!(update_mod_file_content(&path, push_line, None));
        // Idempotent second pass: nothing to add, nothing reported.
        assert!(!update_mod_file_content(&path, "pub mod marker;", None));
        assert!(!update_mod_file_content(&path, push_line, None));
        // Missing file: nothing written, nothing reported.
        assert!(!update_mod_file_content(&dir.join("absent.rs"), "pub mod x;", None));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
