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
    write_python_stubs(output_path);
}

/// The type stubs for the modules the embedded runtime provides, written
/// into the package root (`<lib root>/src`, which pyright searches by
/// default) so editors and type checkers resolve `ndata_py` and `flowlang`.
pub(crate) const NDATA_PY_STUB: &str = include_str!("../ndata_py.pyi");
pub(crate) const FLOWLANG_STUB: &str = include_str!("../flowlang.pyi");

fn write_python_stubs(package_root: &Path) {
    for (name, content) in [("ndata_py.pyi", NDATA_PY_STUB), ("flowlang.pyi", FLOWLANG_STUB)] {
        let path = package_root.join(name);
        if read_to_string(&path).ok().as_deref() != Some(content) {
            let _ = create_dir_all(package_root);
            if let Err(e) = std::fs::write(&path, content) {
                println!("WARNING: could not write Python stub {:?}: {}", path, e);
            }
        }
    }
}

/// The annotation for a declared parameter type. String annotations: never
/// evaluated at runtime, so subprocess mode (where ndata_py does not exist)
/// is unaffected.
fn param_hint(flow_type: &str) -> &'static str {
    match flow_type {
        "String" => "str",
        "Integer" => "int",
        "Float" | "Double" => "float",
        "Boolean" => "bool",
        "JSONObject" => "NDataObject",
        "JSONArray" => "NDataArray",
        "InputStream" => "NDataBytes",
        _ => "Any",
    }
}

/// The annotation for a declared return type: what the command may return
/// (a dict or an NDataObject, a list or an NDataArray, ...).
fn return_hint(flow_type: &str) -> &'static str {
    match flow_type {
        "String" => "str",
        "Integer" => "int",
        "Float" | "Double" => "float",
        "Boolean" => "bool",
        "JSONObject" | "FLAT" => "Mapping[str, Any]",
        "JSONArray" => "Sequence[Any]",
        "InputStream" => "bytes | NDataBytes",
        _ => "Any",
    }
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
        let hint = param_hint(&param_obj.get_string("type"));
        user_fn_param_names.push(format!("{}: \"{}\"", name, hint));
        execute_fn_arg_extraction.push(format!("args['{}']", name));
    }

    let user_fn_params_str = user_fn_param_names.join(", ");
    let execute_args_str = execute_fn_arg_extraction.join(", ");
    let returns = return_hint(&data_section.get_string("returntype"));

    let mut generated_src = String::new();
    generated_src.push_str(&imports);
    generated_src.push_str("\n\n");
    // Names for the annotations, for type checkers only (after the user's
    // imports, so error reports' "imports line N" still count from 1).
    generated_src.push_str("from typing import TYPE_CHECKING\nif TYPE_CHECKING:\n");
    generated_src.push_str("    from collections.abc import Mapping, Sequence\n    from typing import Any\n");
    generated_src.push_str("    from ndata_py import NDataArray, NDataBytes, NDataObject\n\n");
    generated_src.push_str(&format!("def execute(args):\n    return {}({})\n\n", command_name, execute_args_str));
    generated_src.push_str(&format!("def {}({}) -> \"{}\":\n", command_name, user_fn_params_str, returns));

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

#[cfg(test)]
mod tests {
    use super::*;
    use ndata::dataarray::DataArray;
    use std::path::PathBuf;
    use std::process::Command;

    /// Generates `lib/ctl/<cmd>.py` under a fresh package root, as flowb does.
    fn generate(root: &Path, cmd: &str, imports: &str, params: &[(&str, &str)], returntype: &str, body: &str) -> PathBuf {
        crate::builder::test_init();
        let mut meta = DataObject::new();
        let mut data = DataObject::new();
        let mut ps = DataArray::new();
        for (name, typ) in params {
            let mut p = DataObject::new();
            p.put_string("name", name);
            p.put_string("type", typ);
            ps.push_object(p);
        }
        data.put_array("params", ps);
        data.put_string("returntype", returntype);
        data.put_string("import", imports);
        meta.put_object("data", data);
        meta.put_string("cmd", cmd);
        let out = root.join("mylib").join("ctl");
        build_python_command_source(&out, meta, body);
        write_python_stubs(root);
        out.join(format!("{}.py", cmd))
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("flowlang_pybuild_{}_{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d.join("src")
    }

    #[test]
    fn generated_signature_carries_the_declared_types() {
        let root = scratch("sig");
        let params = [("s", "String"), ("i", "Integer"), ("f", "Float"), ("d", "Double"), ("b", "Boolean"),
                      ("o", "JSONObject"), ("a", "JSONArray"), ("x", "Any"), ("st", "InputStream"), ("u", "Unknown")];
        let file = generate(&root, "sig", "import json", &params, "FLAT", "return {}");
        let src = read_to_string(&file).unwrap();
        assert!(src.contains("def sig(s: \"str\", i: \"int\", f: \"float\", d: \"float\", b: \"bool\", o: \"NDataObject\", a: \"NDataArray\", x: \"Any\", st: \"NDataBytes\", u: \"Any\") -> \"Mapping[str, Any]\":\n"), "{}", src);
        // The user's imports stay first: error reports count "imports line N" from 1.
        assert!(src.starts_with("import json\n"), "{}", src);
        for (ret, hint) in [("String", "str"), ("Integer", "int"), ("Boolean", "bool"), ("JSONObject", "Mapping[str, Any]"),
                            ("JSONArray", "Sequence[Any]"), ("InputStream", "bytes | NDataBytes"), ("File", "Any"), ("Any", "Any")] {
            let src = read_to_string(generate(&root, "r", "", &[], ret, "return None")).unwrap();
            assert!(src.contains(&format!("def r() -> \"{}\":\n", hint)), "{}: {}", ret, src);
        }
        assert_eq!(read_to_string(root.join("ndata_py.pyi")).unwrap(), NDATA_PY_STUB);
        assert_eq!(read_to_string(root.join("flowlang.pyi")).unwrap(), FLOWLANG_STUB);
        // String annotations are never evaluated: the module runs without ndata_py.
        if let Ok(out) = Command::new("python3").arg("-c")
            .arg(format!("import runpy; runpy.run_path({:?})", file.display().to_string())).output() {
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        }
    }

    /// With mypy installed (CI installs it), the hints and stubs catch type
    /// errors in a command body and accept a correct one.
    #[test]
    fn mypy_checks_command_bodies_against_the_stubs() {
        if Command::new("mypy").arg("--version").output().map(|o| !o.status.success()).unwrap_or(true) {
            eprintln!("mypy not installed: skipping");
            return;
        }
        let root = scratch("mypy");
        let good = generate(&root, "good", "import flowlang", &[("name", "String"), ("n", "Integer"), ("o", "JSONObject"), ("a", "JSONArray")], "JSONObject",
            "total = flowlang.call('testflow', 'testflow', 'test_add', a=n, b=1)\no['k'] = [name.upper(), n + 1, len(a), a[0], o.get('x', 0), o.get_string('s')]\nreturn {'total': total, 'keys': list(o.keys())}");
        let bad = generate(&root, "bad", "", &[("name", "String"), ("o", "JSONObject")], "Integer",
            "o.put_int('k', name)\nreturn name + 1");
        let run = |f: &PathBuf| Command::new("mypy")
            .args(["--namespace-packages", "--explicit-package-bases", "--no-incremental"])
            .arg(f).env("MYPYPATH", &root).current_dir(root.parent().unwrap()).output().unwrap();
        let g = run(&good);
        assert!(g.status.success(), "{}", String::from_utf8_lossy(&g.stdout));
        let b = run(&bad);
        let out = String::from_utf8_lossy(&b.stdout);
        assert!(!b.status.success(), "{}", out);
        assert!(out.contains("bad.py:") && out.contains("\"put_int\"") && out.contains("Unsupported operand types for +"), "{}", out);
    }
}
