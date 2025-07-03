//! This file will contain the logic for the `rebuild_rust_api` function,
//! which generates a typed Rust API for all registered commands.

use std::collections::HashSet;
use std::fs::{create_dir_all, read_dir};

use crate::DataStore;

use super::util::{get_crate_info, get_project_top_level_path, lookup_rust_api_data_type, lookup_rust_api_ndata_method_suffix};

/// Scans all libraries and commands to generate a typed Rust API.
pub(crate) fn rebuild_rust_api() {
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
                                    
                                    // Only proceed if the command metadata has a 'type' field.
                                    if data_for_cmd_type.has("type") {
                                        if data_for_cmd_type.get_string("type") == "rust" {
                                            let rust_meta_file_id = data_for_cmd_type.get_string("rust");
                                            let rust_cmd_actual_meta = store.get_data(&lib_name, &rust_meta_file_id).get_object("data");
                                            let params_array = rust_cmd_actual_meta.get_array("params");
                                            let rtype_str = rust_cmd_actual_meta.get_string("returntype");
                                            let ntype_ret = lookup_rust_api_ndata_method_suffix(&rtype_str);
                                            let rtype_rust = lookup_rust_api_data_type(&rtype_str);
                                            
                                            let mut params_str_for_fn_def = String::new();
                                            let mut params_setup_str_for_body = String::new();

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
                                            
                                            impl_blocks_str.push_str(&format!("    pub fn {} (&self{}", safe_cmd_name, params_str_for_fn_def));
                                            impl_blocks_str.push_str(&format!(") -> {} {{\n", rtype_rust));
                                            
                                            let d_mut = if params_array.len() > 0 { "mut " } else { "" };
                                            impl_blocks_str.push_str(&format!("        let {}d = ndata::dataobject::DataObject::new();\n", d_mut));
                                            
                                            impl_blocks_str.push_str(&params_setup_str_for_body);
                                            impl_blocks_str.push_str(&format!(
                                                "        flowlang::rustcmd::RustCmd::new(\"{}\").execute(d).expect(\"Rust command execution failed\").get_{}(\"a\")\n    }}\n",
                                                rust_meta_file_id,
                                                ntype_ret
                                            ));
                                        }
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

    // --- FIX for ndata private struct errors ---
    let use_statements = r#"#![allow(non_camel_case_types, unused_variables)]
use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use ndata::databytes::DataBytes;
use ndata::data::Data;
"#;
    // --- End Fix ---

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
            create_dir_all(parent_dir).expect(&format!("Failed to create directory for api.rs: {:?}", parent_dir));
        }
        std::fs::write(&api_file_path, &final_api_code)
            .expect(&format!("Unable to write API file to {:?}", api_file_path));
    }
}
