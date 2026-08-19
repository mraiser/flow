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

    // Builder-managed manifests follow the root's core pair exactly; the root
    // manifest itself is owner-managed input and is never rewritten here.
    if default_package_name != "main_project" {
        if sync_core_pins(&mut lines, lib_name) {
            config_caused_modification = true;
        }
    }

    if cargo_config.has("dependencies") {
        let mut dependencies_map = HashMap::new();
        let dependencies_insertion_line = find_section_insertion_line(&lines, "[dependencies]", &mut dependencies_map);
        let mut new_dependencies = cargo_config.get_object("dependencies").deep_copy();
        for core in ["flowlang", "ndata"] {
            if new_dependencies.has(core) {
                println!("NOTE: {} version for library \"{}\" is builder-managed (the core pair comes from the root Cargo.lock); ignoring the value in meta.json.", core, lib_name);
                new_dependencies.remove_property(core);
            }
        }
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

/// The ndata version this flowlang was built against — the last-resort half of
/// the core pair when neither Cargo.lock nor the root Cargo.toml can name one.
const BUNDLED_NDATA_VERSION: &str = "0.3.17";

/// Produces the flowlang/ndata dependency lines for a newly created manifest,
/// exact-pinned to the resolved core pair.
fn get_core_dependency_lines() -> (String, String) {
    let (flowlang_version, ndata_version) = resolve_core_pair();
    (format!("flowlang = {{ version = \"={}\" }}", flowlang_version),
     format!("ndata = {{ version = \"={}\" }}", ndata_version))
}

/// Determines the single (flowlang, ndata) version pair every builder-managed
/// manifest must depend on. The root Cargo.lock is the authority — it names
/// the exact versions the running binary embodies, so children pinned to it
/// can never drift from the host, in either direction. Fallbacks, in order:
/// versions parsed from the root Cargo.toml's dependency lines, then this
/// flowlang's own compiled-in version with its bundled ndata.
pub(crate) fn resolve_core_pair() -> (String, String) {
    let root = get_project_top_level_path();

    if let Some(pair) = core_pair_from_lock(&root.join("Cargo.lock")) {
        return pair;
    }

    let mut flowlang_version = None;
    let mut ndata_version = None;
    let toml_path = root.join("Cargo.toml");
    if toml_path.exists() {
        if let Ok(lines) = read_lines_from_file(&toml_path) {
            let mut in_deps = false;
            for line in &lines {
                let trimmed = line.trim();
                if trimmed.starts_with('[') {
                    in_deps = trimmed == "[dependencies]";
                    continue;
                }
                if !in_deps || trimmed.contains("path") {
                    continue;
                }
                if trimmed.starts_with("flowlang") && flowlang_version.is_none() {
                    flowlang_version = extract_version_from_dep_line(trimmed);
                } else if trimmed.starts_with("ndata") && ndata_version.is_none() {
                    ndata_version = extract_version_from_dep_line(trimmed);
                }
            }
        }
    }

    if flowlang_version.is_none() || ndata_version.is_none() {
        println!("WARNING: No Cargo.lock and incomplete root Cargo.toml — falling back to this flowlang's own version pair for the core pins.");
    }
    (flowlang_version.unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string()),
     ndata_version.unwrap_or_else(|| BUNDLED_NDATA_VERSION.to_string()))
}

/// Reads the exact resolved flowlang/ndata versions out of a Cargo.lock.
/// Returns None unless both are present. If the graph somehow holds several
/// versions of one of them, the highest wins — that is the pair a fresh
/// resolution would converge on.
fn core_pair_from_lock(lock_path: &PathBuf) -> Option<(String, String)> {
    if !lock_path.exists() {
        return None;
    }
    let lines = read_lines_from_file(lock_path).ok()?;
    let mut flowlang_versions: Vec<String> = Vec::new();
    let mut ndata_versions: Vec<String> = Vec::new();
    let mut current_name: Option<String> = None;
    for line in &lines {
        let trimmed = line.trim();
        if trimmed == "[[package]]" {
            current_name = None;
        } else if let Some(rest) = trimmed.strip_prefix("name = ") {
            current_name = Some(rest.trim_matches('"').to_string());
        } else if let Some(rest) = trimmed.strip_prefix("version = ") {
            let version = rest.trim_matches('"').to_string();
            match current_name.as_deref() {
                Some("flowlang") => flowlang_versions.push(version),
                Some("ndata") => ndata_versions.push(version),
                _ => {}
            }
        }
    }
    Some((pick_highest_version(flowlang_versions)?, pick_highest_version(ndata_versions)?))
}

fn pick_highest_version(mut versions: Vec<String>) -> Option<String> {
    versions.sort_by_key(|v| {
        v.split(|c: char| !c.is_ascii_digit())
            .map(|seg| seg.parse::<u64>().unwrap_or(0))
            .collect::<Vec<u64>>()
    });
    versions.pop()
}

/// Pulls the bare version out of a dependency line, whichever shape it takes:
/// `ndata = "0.3.17"` or `flowlang = { version = "0.3.32", ... }`. Any
/// requirement operator (=, ^, ~) is stripped.
fn extract_version_from_dep_line(line: &str) -> Option<String> {
    let (start, end) = find_version_quotes(line)?;
    Some(line[start + 1..end].trim_start_matches(['=', '^', '~']).to_string())
}

/// Locates the version's quoted string in a dependency line, returning the
/// byte offsets of its opening and closing quotes.
fn find_version_quotes(line: &str) -> Option<(usize, usize)> {
    let search_from = if line.contains('{') {
        let brace = line.find('{')?;
        brace + line[brace..].find("version")?
    } else {
        line.find('=')?
    };
    let open = search_from + line[search_from..].find('"')?;
    let close = open + 1 + line[open + 1..].find('"')?;
    Some((open, close))
}

/// Rewrites the flowlang/ndata dependency lines of a builder-managed manifest
/// to exact-match pins on the resolved core pair — in both directions, so a
/// rolled-back root pulls newer children back down too. Only the version
/// string inside the line is touched; features and the rest of the line's
/// shape survive. Path dependencies are left alone.
fn sync_core_pins(lines: &mut Vec<String>, lib_name: &str) -> bool {
    let (flowlang_version, ndata_version) = resolve_core_pair();
    pin_core_lines(lines, &flowlang_version, &ndata_version, lib_name)
}

fn pin_core_lines(lines: &mut Vec<String>, flowlang_version: &str, ndata_version: &str, lib_name: &str) -> bool {
    let mut modified = false;
    let mut in_deps = false;
    for line in lines.iter_mut() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_deps = trimmed == "[dependencies]";
            continue;
        }
        if !in_deps || trimmed.contains("path") {
            continue;
        }
        let key = match trimmed.split('=').next() {
            Some(k) => k.trim(),
            None => continue,
        };
        let pin = match key {
            "flowlang" => format!("={}", flowlang_version),
            "ndata" => format!("={}", ndata_version),
            _ => continue,
        };
        match find_version_quotes(line) {
            Some((open, close)) => {
                if &line[open + 1..close] != pin {
                    let new_line = format!("{}{}{}", &line[..open + 1], pin, &line[close..]);
                    println!("Core pin: {} -> \"{}\" in Cargo.toml for library \"{}\" (was: {})", key, pin, lib_name, line.trim());
                    *line = new_line;
                    modified = true;
                }
            }
            None => {
                println!("WARNING: Could not locate a version string in the {} dependency line for library \"{}\"; leaving it alone: {}", key, lib_name, line.trim());
            }
        }
    }
    modified
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

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(s: &str) -> Vec<String> {
        s.lines().map(|l| l.to_string()).collect()
    }

    #[test]
    fn extracts_versions_from_both_dep_line_shapes() {
        assert_eq!(extract_version_from_dep_line(r#"ndata = "0.3.17""#).as_deref(), Some("0.3.17"));
        assert_eq!(extract_version_from_dep_line(r#"flowlang = { version = "0.3.32" }"#).as_deref(), Some("0.3.32"));
        assert_eq!(extract_version_from_dep_line(r#"flowlang = { version="=0.3.32", features = ["serde_support"] }"#).as_deref(), Some("0.3.32"));
        assert_eq!(extract_version_from_dep_line(r#"flowlang = "^0.3.31""#).as_deref(), Some("0.3.31"));
    }

    #[test]
    fn picks_the_highest_version_numerically() {
        let versions = vec!["0.3.9".to_string(), "0.3.32".to_string(), "0.3.10".to_string()];
        assert_eq!(pick_highest_version(versions).as_deref(), Some("0.3.32"));
        assert_eq!(pick_highest_version(vec![]), None);
    }

    #[test]
    fn pins_both_directions_and_preserves_line_shape() {
        // agent skewed down, kb-style plain form skewed up: both converge.
        let mut manifest = lines(concat!(
            "[package]\n",
            "name = \"agent\"\n",
            "\n",
            "[dependencies]\n",
            "flowlang = { version = \"0.3.30\", features = [\"serde_support\"] }\n",
            "ndata = \"0.3.99\"\n",
            "serde = { version = \"1.0\", optional = true }\n",
        ));
        assert!(pin_core_lines(&mut manifest, "0.3.32", "0.3.17", "agent"));
        assert_eq!(manifest[4], r#"flowlang = { version = "=0.3.32", features = ["serde_support"] }"#);
        assert_eq!(manifest[5], r#"ndata = "=0.3.17""#);
        assert_eq!(manifest[6], r#"serde = { version = "1.0", optional = true }"#);
        // Second pass is a no-op: already at the pins.
        assert!(!pin_core_lines(&mut manifest, "0.3.32", "0.3.17", "agent"));
    }

    #[test]
    fn leaves_path_deps_and_other_sections_alone() {
        let original = concat!(
            "[dependencies]\n",
            "flowlang = { path = \"../flow\" }\n",
            "[dev-dependencies]\n",
            "ndata = \"0.3.16\"\n",
        );
        let mut manifest = lines(original);
        assert!(!pin_core_lines(&mut manifest, "0.3.32", "0.3.17", "x"));
        assert_eq!(manifest, lines(original));
    }

    #[test]
    fn reads_the_pair_out_of_a_lock_file() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("flowlang-corepair-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let lock = dir.join("Cargo.lock");
        let mut f = std::fs::File::create(&lock).unwrap();
        write!(f, concat!(
            "version = 3\n\n",
            "[[package]]\n",
            "name = \"flowlang\"\n",
            "version = \"0.3.32\"\n",
            "source = \"registry+https://github.com/rust-lang/crates.io-index\"\n\n",
            "[[package]]\n",
            "name = \"ndata\"\n",
            "version = \"0.3.17\"\n",
        )).unwrap();
        assert_eq!(core_pair_from_lock(&lock), Some(("0.3.32".to_string(), "0.3.17".to_string())));
        assert_eq!(core_pair_from_lock(&dir.join("nope.lock")), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
