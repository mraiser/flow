//! This file will contain the logic for the `rebuild_rust_api` function,
//! which generates a typed Rust API for all registered commands.
//!
//! The generated file carries two shapes of the same API:
//!
//! * a module tree of stateless functions (`api::gudrun::admin::list_jobs(...)`)
//!   — the current form; and
//! * the original struct facade (`api::new().gudrun.admin.list_jobs(...)`),
//!   kept one release for migration with `#[deprecated]` on every method.
//!   Its internal structs are renamed `old_*` because module names and type
//!   names share a namespace — `pub mod gudrun` and `pub struct gudrun`
//!   cannot coexist (E0428). Callers never name the internal structs (they
//!   reach them through field access), so the rename is invisible; only
//!   `struct api`, `new()`, and the field names are public surface, and those
//!   are unchanged and deliberately NOT deprecated — generated `lib.rs` files
//!   hold `pub static API: api = new();`, and deprecating the type or the
//!   constructor would warn on lines users don't own. Method call sites are
//!   user code, so the methods carry the deprecation.
//!
//! Every reference to a dependency crate in generated code is `::`-anchored
//! (`::flowlang::rustcmd::RustCmd`, `use ::ndata::...`). This is load-bearing:
//! a library named `flowlang` exists in most stores, so the generated
//! `pub mod flowlang` (previously `pub struct flowlang`) shadows the bare
//! crate path — the struct form failed with E0223, and a glob import of the
//! module form resolves `flowlang::rustcmd` into the generated module (E0433).
//! A leading `::` can only mean the crate.

use std::collections::HashSet;
use std::fs::{create_dir_all, read_dir};

use crate::DataStore;

use super::util::{get_crate_info, get_project_top_level_path, lookup_rust_api_data_type, lookup_rust_api_ndata_method_suffix};

/// One rust command's metadata, resolved from the store.
struct ApiCommand {
    name: String,
    impl_id: String,
    /// (name, rust type, ndata method suffix) per declared param
    params: Vec<(String, String, String)>,
    return_rust_type: String,
    return_suffix: String,
}

/// Scans all libraries and commands to generate a typed Rust API, writing
/// `src/api.rs` into every sub-crate (libraries whose root is not ".").
pub(crate) fn rebuild_rust_api() {
    let store = DataStore::new();
    let project_top_level_path = get_project_top_level_path();

    let mut crates = HashSet::new();
    let mut lib_names = Vec::new();
    let lib_entries = read_dir("data")
        .expect("Failed to read 'data' directory for API rebuilding.");
    for dir_entry_result in lib_entries {
        let dir_entry = dir_entry_result.expect("Error reading a directory entry in 'data'.");
        let lib_name = dir_entry.file_name().into_string().expect("Library name is not valid UTF-8.");
        let lib_metadata = store.lib_info(&lib_name);
        let (root_name, _) = get_crate_info(&lib_metadata);
        if root_name != "." {
            crates.insert(root_name);
        }
        if store.exists(&lib_name, "controls") {
            lib_names.push(lib_name);
        }
    }
    // read_dir order is filesystem-dependent; sort so the output is stable.
    lib_names.sort();

    let final_api_code = generate_api_code(&store, &lib_names);

    for crate_name in crates {
        let api_file_path = project_top_level_path.join(crate_name).join("src").join("api.rs");
        if let Some(parent_dir) = api_file_path.parent() {
            create_dir_all(parent_dir).expect(&format!("Failed to create directory for api.rs: {:?}", parent_dir));
        }
        std::fs::write(&api_file_path, &final_api_code)
            .expect(&format!("Unable to write API file to {:?}", api_file_path));
    }
}

/// Builds the complete `api.rs` source for the given libraries.
///
/// Public so an external harness can generate against any store and compile
/// the result; `rebuild_rust_api` is this plus the write-to-sub-crates loop.
pub fn generate_api_code(store: &DataStore, lib_names: &[String]) -> String {
    let mut modules = String::new();
    let mut wrapper_structs = String::new();
    let mut lib_structs = String::new();
    let mut api_fields = String::new();
    let mut api_init = String::new();
    let mut impls = String::new();

    for lib_name in lib_names {
        let safe_lib = lib_name.replace("-", "_");
        modules.push_str(&format!("pub mod {} {{\n", safe_lib));
        api_fields.push_str(&format!("    pub {}: old_{},\n", safe_lib, safe_lib));
        api_init.push_str(&format!("        {}: old_{} {{\n", safe_lib, safe_lib));
        lib_structs.push_str(&format!("pub struct old_{} {{\n", safe_lib));

        let controls_data = store.get_data(lib_name, "controls");
        let list = controls_data.get_object("data").get_array("list");

        for control_val in list.objects() {
            let control = control_val.object();
            let ctl_name = control.get_string("name");
            let safe_ctl = ctl_name.replace("-", "_");
            let ctl_id = control.get_string("id");
            if !store.exists(lib_name, &ctl_id) {
                continue;
            }

            let struct_name = format!("old_{}_{}", safe_lib, safe_ctl);
            api_init.push_str(&format!("            {}: {} {{}},\n", safe_ctl, struct_name));
            lib_structs.push_str(&format!("    pub {}: {},\n", safe_ctl, struct_name));
            wrapper_structs.push_str(&format!("pub struct {} {{}}\n", struct_name));

            // A record store.get_data cannot parse (e.g. ndata 0.3.16's json
            // parser rejects every negative number — a scene facet with
            // "x":-2.4 makes its whole control record unreadable) must not
            // abort the API build for every other library: skip the control,
            // say so, and keep going. Its commands are absent from the API
            // until the record parses again.
            let commands = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || collect_rust_commands(store, lib_name, &ctl_id)))
                .unwrap_or_else(|_| {
                    eprintln!("WARNING: skipping API for {}:{} — a record in its chain does not parse",
                              lib_name, ctl_name);
                    Vec::new()
                });

            // ---- module half ----
            modules.push_str(&format!("    pub mod {} {{\n", safe_ctl));
            modules.push_str("        use ::ndata::dataobject::DataObject;\n");
            modules.push_str("        use ::ndata::dataarray::DataArray;\n");
            modules.push_str("        use ::ndata::databytes::DataBytes;\n");
            modules.push_str("        use ::ndata::data::Data;\n\n");
            for cmd in &commands {
                let params_sig = cmd.params.iter()
                    .map(|(n, t, _)| format!("{}: {}", n, t))
                    .collect::<Vec<_>>()
                    .join(", ");
                modules.push_str(&format!("        pub fn {}({}) -> {} {{\n",
                    cmd.name, params_sig, cmd.return_rust_type));
                let d_mut = if cmd.params.is_empty() { "" } else { "mut " };
                modules.push_str(&format!("            let {}d = DataObject::new();\n", d_mut));
                for (pname, ptype, psuffix) in &cmd.params {
                    let amp = if ptype == "String" { "&" } else { "" };
                    let method_prefix = if psuffix == "property" { "set" } else { "put" };
                    modules.push_str(&format!("            d.{}_{}(\"{}\", {}{});\n",
                        method_prefix, psuffix, pname, amp, pname));
                }
                modules.push_str(&format!(
                    "            ::flowlang::rustcmd::RustCmd::new(\"{}\").execute(d).expect(\"Rust command execution failed\").get_{}(\"a\")\n        }}\n\n",
                    cmd.impl_id, cmd.return_suffix));
            }
            modules.push_str("    }\n");

            // ---- deprecated facade half: methods delegate to the module fns ----
            if !commands.is_empty() {
                impls.push_str(&format!("impl {} {{\n", struct_name));
                for cmd in &commands {
                    let params_sig = cmd.params.iter()
                        .map(|(n, t, _)| format!(", {}: {}", n, t))
                        .collect::<String>();
                    let args = cmd.params.iter()
                        .map(|(n, _, _)| n.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    impls.push_str(&format!(
                        "    #[deprecated(note = \"use api::{}::{}::{} instead\")]\n",
                        safe_lib, safe_ctl, cmd.name));
                    impls.push_str(&format!("    pub fn {}(&self{}) -> {} {{\n",
                        cmd.name, params_sig, cmd.return_rust_type));
                    impls.push_str(&format!("        self::{}::{}::{}({})\n    }}\n",
                        safe_lib, safe_ctl, cmd.name, args));
                }
                impls.push_str("}\n");
            }
        }

        modules.push_str("}\n\n");
        api_init.push_str("        },\n");
        lib_structs.push_str("}\n");
    }

    format!(
        "{}{}{}{}pub struct api {{\n{}}}\n\npub const fn new() -> api {{\n    api {{\n{}    }}\n}}\n\n{}",
        HEADER, modules, wrapper_structs, lib_structs, api_fields, api_init, impls
    )
}

const HEADER: &str = r#"#![allow(non_camel_case_types, unused_variables, unused_imports, dead_code)]
pub use ::ndata::dataobject::DataObject;
pub use ::ndata::dataarray::DataArray;
pub use ::ndata::databytes::DataBytes;
pub use ::ndata::data::Data;

"#;

/// Resolves a control's rust-typed commands to their signatures.
fn collect_rust_commands(store: &DataStore, lib_name: &str, ctl_id: &str) -> Vec<ApiCommand> {
    let mut commands = Vec::new();
    let ctldata = store.get_data(lib_name, ctl_id);
    let d = ctldata.get_object("data");
    if !d.has("cmd") {
        return commands;
    }
    for command_val in d.get_array("cmd").objects() {
        let command = command_val.object();
        let cmd_id = command.get_string("id");
        if !store.exists(lib_name, &cmd_id) {
            continue;
        }
        let cmd_data = store.get_data(lib_name, &cmd_id).get_object("data");
        if !cmd_data.has("type") || cmd_data.get_string("type") != "rust" {
            continue;
        }
        let impl_id = cmd_data.get_string("rust");
        let impl_data = store.get_data(lib_name, &impl_id).get_object("data");
        let rtype = impl_data.get_string("returntype");
        let mut params = Vec::new();
        for param_val in impl_data.get_array("params").objects() {
            let param = param_val.object();
            let ptype = param.get_string("type");
            params.push((
                param.get_string("name"),
                lookup_rust_api_data_type(&ptype).to_string(),
                lookup_rust_api_ndata_method_suffix(&ptype).to_string(),
            ));
        }
        commands.push(ApiCommand {
            name: command.get_string("name").replace("-", "_"),
            impl_id,
            params,
            return_rust_type: lookup_rust_api_data_type(&rtype).to_string(),
            return_suffix: lookup_rust_api_ndata_method_suffix(&rtype).to_string(),
        });
    }
    commands
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generated_for_this_repo() -> String {
        crate::builder::test_init();
        let store = DataStore::new();
        // this repo's store contains a library literally named "flowlang" —
        // the shadowing case that broke the struct-based generator (E0223)
        let libs = vec!["flowlang".to_string(), "mcp".to_string(), "testflow".to_string()];
        generate_api_code(&store, &libs)
    }

    #[test]
    fn api_output_has_both_halves_and_anchored_paths() {
        let src = generated_for_this_repo();

        // module half and facade half both present
        assert!(src.contains("pub mod flowlang {"), "module tree missing");
        assert!(src.contains("pub struct api {"), "facade struct missing");
        assert!(src.contains("pub const fn new() -> api {"), "facade constructor missing");

        // facade internals renamed: nothing may claim the module names
        assert!(src.contains("pub struct old_flowlang {"), "renamed lib struct missing");
        assert!(!src.contains("pub struct flowlang"),
                "bare `pub struct flowlang` collides with `pub mod flowlang` (E0428)");

        // every dependency-crate path is ::-anchored; a bare `flowlang::` or
        // `ndata::` path resolves into the generated items instead (E0223/E0433)
        for needle in ["flowlang::rustcmd::RustCmd", "ndata::dataobject::DataObject",
                       "ndata::dataarray::DataArray", "ndata::databytes::DataBytes",
                       "ndata::data::Data"] {
            for (i, _) in src.match_indices(needle) {
                assert!(i >= 2 && &src[i - 2..i] == "::",
                        "un-anchored crate path at byte {}: ...{}", i,
                        &src[i.saturating_sub(20)..(i + needle.len()).min(src.len())]);
            }
        }
        assert!(!src.contains("use super::super::*"),
                "glob import would pull generated modules over the extern crates");

        // deprecation lands on methods, naming the replacement path
        assert!(src.contains("#[deprecated(note = \"use api::flowlang::http::hex_encode instead\")]"));

        // a known command appears as a plain function with its real signature
        assert!(src.contains("pub fn hex_encode(a: String) -> String {"));
    }

    #[test]
    #[ignore = "writes a temp crate depending on this checkout and cargo-checks the generated API; run with -- --ignored"]
    fn api_output_compiles() {
        let src = generated_for_this_repo();
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let dir = std::env::temp_dir().join("flowlang-apicheck");
        let src_dir = dir.join("src");
        create_dir_all(&src_dir).unwrap();
        std::fs::write(dir.join("Cargo.toml"), format!(
            "[package]\nname = \"apicheck\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n\
             [dependencies]\nflowlang = {{ path = \"{}\" }}\nndata = \"0.3.16\"\n\
             [workspace]\n", manifest_dir)).unwrap();
        std::fs::write(src_dir.join("lib.rs"), "pub mod api;\n").unwrap();
        std::fs::write(src_dir.join("api.rs"), &src).unwrap();

        let out = std::process::Command::new("cargo")
            .arg("check")
            .current_dir(&dir)
            .env("CARGO_TARGET_DIR", dir.join("target"))
            .output()
            .expect("failed to run cargo");
        assert!(out.status.success(),
                "generated api.rs does not compile:\n{}",
                String::from_utf8_lossy(&out.stderr));
    }
}
