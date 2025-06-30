//! This file handles the generation of all Rust-specific source code
//! for flowlang commands.

use std::collections::HashSet;
use std::fs::{create_dir_all, read_to_string};
use std::path::Path;

use ndata::dataarray::DataArray;
use ndata::dataobject::DataObject;

use crate::DataStore;

use super::util::{update_mod_file_content, lookup_rust_api_data_type};

/// Orchestrates the generation of a single Rust command's source file.
///
/// It generates the source code from metadata and writes it to a file if it has changed.
/// It then updates the relevant `mod.rs` files to include this new command module.
/// Returns true if a file was written, indicating a change.
pub(crate) fn build_rust_command(
    output_path: &Path,
    lib_name: &str,
    control_name: &str,
    command_name: &str,
    store: &DataStore,
) -> bool {
    let mut artifact_changed = false;
    let command_id = store.lookup_cmd_id(lib_name, control_name, command_name);

    if !store.exists(lib_name, &command_id) {
        println!("Command ID {} not found for {}:{}:{}", command_id, lib_name, control_name, command_name);
        return false;
    }

    let command_metadata = store.get_data(lib_name, &command_id);
    let data_section = command_metadata.get_object("data");
    let rust_file_id = data_section.get_string("rust");

    let mut rust_meta = store.get_data(lib_name, &rust_file_id);
    let source_file_path = store.get_data_file(lib_name, &(rust_file_id.clone() + ".rs"));
    let source_code = store.read_file(source_file_path);

    // Inject context into the metadata before generation
    rust_meta.put_string("lib", lib_name);
    rust_meta.put_string("ctl", control_name);
    rust_meta.put_string("cmd", command_name);

    let command_and_control_output_path = output_path.join(lib_name).join(control_name);

    if build_rust_command_source(&command_and_control_output_path, rust_meta, &source_code) {
        artifact_changed = true;
    }

    // After generating the source, update the module tree
    build_mod_files_for_rust_command(
        &command_and_control_output_path,
        control_name,
        command_name,
        &rust_file_id,
    );

    artifact_changed
}

/// Generates and writes the Rust source file for a command.
/// Returns true if the file was written (i.e., content was new or changed).
fn build_rust_command_source(
    output_path: &Path,
    meta: DataObject,
    template_code: &str,
) -> bool {
    let command_name = meta.get_string("cmd");
    let generated_src = generate_rust_source_from_meta(meta, template_code);
    let rust_output_file = output_path.join(format!("{}.rs", command_name));

    // Only write the file if it doesn't exist or the content has changed.
    let mut needs_write = true;
    if rust_output_file.exists() {
        if let Ok(old_src) = read_to_string(&rust_output_file) {
            if old_src == generated_src {
                needs_write = false;
            }
        }
    }

    if needs_write {
        create_dir_all(output_path).expect("Failed to create directory for rust command source");
        std::fs::write(&rust_output_file, generated_src)
            .expect(&format!("Unable to write Rust file: {:?}", rust_output_file));
        return true;
    }
    false
}

/// Updates the `mod.rs` files at the control and library level to include the new command.
fn build_mod_files_for_rust_command(
    command_output_path: &Path,
    control_name: &str,
    command_name: &str,
    rust_cmd_meta_id: &str,
) {
    // Update the control's mod.rs (e.g., src/my_lib/my_control/mod.rs)
    let ctl_mod_file = command_output_path.join("mod.rs");
    // It's assumed ensure_mod_file_has_cmdinit was already called during scaffolding.
    update_mod_file_content(&ctl_mod_file, &format!("pub mod {};", command_name), None);
    update_mod_file_content(&ctl_mod_file, &format!("cmds.push((\"{}\".to_string(), {}::execute, \"\".to_string()));", rust_cmd_meta_id, command_name), None);

    // Update the library's mod.rs (e.g., src/my_lib/mod.rs)
    let lib_mod_path = command_output_path.parent()
        .expect("Command output path should have a parent (library module level)");
    let lib_mod_file = lib_mod_path.join("mod.rs");
    update_mod_file_content(&lib_mod_file, &format!("pub mod {};", control_name), None);
    update_mod_file_content(&lib_mod_file, &format!("{}::cmdinit(cmds);", control_name), None);
}

/// Generates the full Rust source code string for a command from its metadata and user code.
fn generate_rust_source_from_meta(meta: DataObject, user_code: &str) -> String {
    let data_section = meta.get_object("data");
    let command_name = meta.get_string("cmd");
    let user_provided_imports = data_section.get_string("import");
    let returntype_meta_str = data_section.get_string("returntype");
    let params_array = data_section.get_array("params");

    let mut src = String::new();

    // Ensure required ndata imports are present
    let dataobject_import = "use ndata::dataobject::DataObject;";
    if !user_provided_imports.contains(dataobject_import) {
        src.push_str(dataobject_import);
        src.push('\n');
    }

    let mut wrapper_ndata_types_needed = HashSet::new();
    for param_value in params_array.objects() {
        let param_obj = param_value.object();
        let meta_type = param_obj.get_string("type");
        let rust_type = lookup_rust_api_data_type(&meta_type);
        if ["DataArray", "DataBytes", "Data"].contains(&rust_type) {
            wrapper_ndata_types_needed.insert(rust_type.to_string());
        }
    }
    let rust_return_type = lookup_rust_api_data_type(&returntype_meta_str);
    if ["DataArray", "DataBytes", "Data"].contains(&rust_return_type) {
        wrapper_ndata_types_needed.insert(rust_return_type.to_string());
    }

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
    src.push('\n');

    // Generate the `execute` wrapper function
    let execute_param_name = if params_array.len() == 0 { "_" } else { "o" };
    src.push_str(&format!("pub fn execute({}: DataObject) -> DataObject {{\n", execute_param_name));

    let (param_extraction_code, function_call_args, user_fn_param_defs) =
        generate_rust_invoke_parts(params_array.clone());
    let return_packaging_code = generate_rust_return_packaging(&rust_return_type);

    src.push_str(&param_extraction_code);
    src.push_str(&format!("    let ax = {}({});\n", command_name, function_call_args));
    src.push_str("    let mut result_obj = DataObject::new();\n");
    src.push_str(&return_packaging_code);
    src.push_str("    result_obj\n");
    src.push_str("}\n\n");

    // Append the user's actual function code
    src.push_str(&format!("pub fn {}({}) -> {} {{\n", command_name, user_fn_param_defs, rust_return_type));
    src.push_str(user_code);
    src.push_str("\n}\n");
    src
}

/// Generates the code snippets for parameter handling in the `execute` wrapper.
fn generate_rust_invoke_parts(params: DataArray) -> (String, String, String) {
    let mut param_extraction_code = String::new();
    let mut function_call_args = String::new();
    let mut user_fn_param_defs = String::new();

    for (index, param_value) in params.objects().iter().enumerate() {
        let param_obj = param_value.object();
        let name = param_obj.get_string("name");
        let meta_type = param_obj.get_string("type");
        let rust_type = lookup_rust_api_data_type(&meta_type);
        let arg_var_name = format!("arg_{}", index);

        let getter_suffix = match rust_type {
            "DataObject" => "object", "DataArray" => "array", "DataBytes" => "bytes",
            "Data" => "property", "bool" => "boolean", "i64" => "int",
            "f64" => "float", "String" => "string", _ => "property",
        };
        param_extraction_code.push_str(&format!(
            "    let {}: {} = o.get_{}(\"{}\");\n", arg_var_name, rust_type, getter_suffix, name));

        if index > 0 {
            function_call_args.push_str(", ");
            user_fn_param_defs.push_str(", ");
        }
        function_call_args.push_str(&arg_var_name);
        user_fn_param_defs.push_str(&format!("{}: {}", name, rust_type));
    }
    (param_extraction_code, function_call_args, user_fn_param_defs)
}

/// Generates the code snippet for packaging the return value into a `DataObject`.
fn generate_rust_return_packaging(rust_return_type: &str) -> String {
    if rust_return_type == "Data" {
        "    result_obj.set_property(\"a\", ax);\n".to_string()
    } else {
        let putter_suffix = match rust_return_type {
            "String" => "string(\"a\", &ax)",
            "f64" => "float(\"a\", ax)",
            "i64" => "int(\"a\", ax)",
            "bool" => "boolean(\"a\", ax)",
            "DataObject" => "object(\"a\", ax)",
            "DataArray" => "array(\"a\", ax)",
            "DataBytes" => "bytes(\"a\", ax)",
            _ => {
                eprintln!("Warning: Unhandled Rust return type for packaging: {}", rust_return_type);
                // The `Data` type is needed for this fallback case.
                "property(\"a\", ndata::data::Data::from(ax))"
            }
        };
        format!("    result_obj.put_{};\n", putter_suffix)
    }
}
