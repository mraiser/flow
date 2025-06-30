//! This file handles the generation of all Python-specific source code
//! for flowlang commands.

use std::fs::{create_dir_all, read_to_string};
use std::io::{BufRead, BufReader};
use std::path::Path;

use ndata::dataobject::DataObject;

use crate::DataStore;

/// Orchestrates the generation of a single Python command's source file.
pub(crate) fn build_python_command(
    output_path: &Path,
    lib_name: &str,
    control_name: &str,
    command_name: &str,
    store: &DataStore,
) {
    let command_id = store.lookup_cmd_id(lib_name, control_name, command_name);

    if !store.exists(lib_name, &command_id) {
        println!("Command ID {} not found for {}:{}:{}", command_id, lib_name, control_name, command_name);
        return;
    }

    let command_metadata = store.get_data(lib_name, &command_id);
    let data_section = command_metadata.get_object("data");
    let python_file_id = data_section.get_string("python");

    let mut python_meta = store.get_data(lib_name, &python_file_id);
    let source_file_path = store.get_data_file(lib_name, &(python_file_id.clone() + ".python"));
    let source_code = store.read_file(source_file_path);

    // Inject context into the metadata before generation
    python_meta.put_string("lib", lib_name);
    python_meta.put_string("ctl", control_name);
    python_meta.put_string("cmd", command_name);

    let command_and_control_output_path = output_path.join(lib_name).join(control_name);

    build_python_command_source(&command_and_control_output_path, python_meta, &source_code);
}

/// Generates and writes the Python source file for a command.
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
    generated_src.push_str(&imports);
    generated_src.push_str("\n\n");
    generated_src.push_str(&format!("def execute(args):\n    return {}({})\n\n", command_name, execute_args_str));
    generated_src.push_str(&format!("def {}({}):\n", command_name, user_fn_params_str));

    // Indent user code
    for line in BufReader::new(user_code.as_bytes()).lines() {
        generated_src.push_str(&format!("    {}\n", line.expect("Failed to read line from Python user code")));
    }

    // Add a main execution block for testing
    generated_src.push_str("\n\nif __name__ == \"__main__\":\n");
    generated_src.push_str("    import sys\n    import json\n");
    generated_src.push_str("    print(json.dumps(execute(json.loads(sys.argv[1]))))\n");

    // Only write the file if it doesn't exist or the content has changed.
    let mut needs_write = true;
    if python_output_file.exists() {
        if let Ok(old_src) = read_to_string(&python_output_file) {
            if old_src == generated_src {
                needs_write = false;
            }
        }
    }

    if needs_write {
        create_dir_all(output_path).expect("Failed to create directory for python command source");
        std::fs::write(&python_output_file, generated_src)
            .expect(&format!("Unable to write Python file: {:?}", python_output_file));
    }
}
