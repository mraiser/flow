// pyenv.rs

use ndata::dataobject::DataObject;
use ndata::data::Data;
use std::sync::{Once, RwLock};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use std::path::Path;
use crate::DataStore;
use crate::pycmd::PyCmd;
use crate::pywrapper::NDataObjectPy;


static START: Once = Once::new();
static PYENV: RwLock<Option<PyEnv>> = RwLock::new(None);

pub struct PyEnv;

impl PyEnv {
    pub fn new(venv_path: Option<&str>) -> PyEnv {
        if let Some(path) = venv_path {
            if !Path::new(path).exists() {
                panic!("Virtual environment not found at {}", path);
            }
            PyEnv::activate_venv(path);
        }
        PyEnv
    }

    fn activate_venv(venv_path: &str) {
        let _venv_bin = format!("{}/bin/python", venv_path);
        std::env::set_var("PYTHONHOME", "");
        std::env::set_var("PYTHONPATH", "");
        std::env::set_var("PATH", format!("{}/bin:{}", venv_path, std::env::var("PATH").unwrap_or_default()));
        std::env::set_var("VIRTUAL_ENV", venv_path);
    }

    // Added returntype parameter
    pub fn execute_function(&self, file_path: &str, name: &str, params: DataObject, returntype: &str) -> PyResult<DataObject> {
        let base_path = "cmd/src";
        let module_name = compute_module_name(base_path, file_path);

        Python::with_gil(|py| {
            let sys_module = py.import_bound("sys")?;
            let binding = sys_module.getattr("path")?;
            let sys_path: &Bound<'_, PyList> = binding.downcast()?;

            if !sys_path.contains(base_path)? {
                sys_path.insert(0, base_path)?;
            }

            if let Some(file_dir) = Path::new(file_path).parent() {
                if let Some(file_dir_str) = file_dir.to_str() {
                    if !sys_path.contains(file_dir_str)? {
                        sys_path.insert(0, file_dir_str)?;
                    }
                }
            }

            let module = py.import_bound(module_name.as_str())?;
            let function = module.getattr(name)?;

            let kwargs = PyDict::new_bound(py);
            for (key, data_value) in params.objects() {
                let py_value = crate::pywrapper::data_to_py_any(py, data_value.clone());
                kwargs.set_item(key, py_value)?;
            }

            let binding = function.call((), Some(&kwargs))?;
            let result_any: &Bound<'_, PyAny> = binding.as_ref();

            // --- NEW RESULT HANDLING BASED ON RETURNTYPE ---

            if returntype == "FLAT" {
                // If returntype is FLAT, we expect the Python function to return
                // an NDataObjectPy or a PyDict that can be converted to one.
                if let Ok(wrapped_result) = result_any.extract::<PyRef<NDataObjectPy>>() {
                    let final_obj = wrapped_result.0.clone();
                    // For FLAT, the Python side is responsible for the full structure.
                    // We might optionally ensure 'status' if it's a convention.
                    // if !final_obj.has("status") { final_obj.put_string("status", "ok"); }
                    return Ok(final_obj);
                }
                if let Ok(py_dict) = result_any.downcast::<PyDict>() {
                    let mut rust_do = DataObject::new();
                    for (key_any, value_any) in py_dict.iter() {
                        let key_str: String = key_any.extract()?;
                        let data_val = crate::pywrapper::py_any_to_data(&value_any)?;
                        rust_do.set_property(&key_str, data_val);
                    }
                    // if !rust_do.has("status") { rust_do.put_string("status", "ok"); }
                    return Ok(rust_do);
                }
                // If returntype is FLAT and Python returned something else, it's a contract violation.
                return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(format!(
                    "Python function '{}' was expected to return a DataObject-like structure (for FLAT returntype), but returned type: {}",
                    name,
                    result_any.get_type().name()?
                )));
            }

            // For non-FLAT returntypes:

            // Case 1: Python function returns an NDataObjectPy instance directly.
            // This means the Python function chose to return a pre-structured DataObject. We respect it.
            if let Ok(wrapped_result) = result_any.extract::<PyRef<NDataObjectPy>>() {
                let mut final_obj = wrapped_result.0.clone();
                if !final_obj.has("status") {
                    final_obj.put_string("status", "ok");
                }
                return Ok(final_obj);
            }

            // Case 2: Python function returns a raw PyDict.
            // Similar to above, Python returned a pre-structured dict. Convert and respect it.
            if let Ok(py_dict) = result_any.downcast::<PyDict>() {
                let mut rust_do = DataObject::new();
                for (key_any, value_any) in py_dict.iter() {
                    let key_str: String = key_any.extract()?;
                    let data_val = crate::pywrapper::py_any_to_data(&value_any)?;
                    rust_do.set_property(&key_str, data_val);
                }
                if !rust_do.has("status") {
                     rust_do.put_string("status", "ok");
                }
                return Ok(rust_do);
            }

            // Case 3: Python function returns None.
            // Wrap this in a DataObject according to returntype.
            if result_any.is_none() {
                 let mut rust_do = DataObject::new();
                 rust_do.put_string("status", "ok");
                 let key_to_use = if returntype == "String" { "msg" } else { "data" };
                 rust_do.put_null(key_to_use);
                 return Ok(rust_do);
            }

            // Case 4: Python function returns any other simple type.
            // Convert to ndata::Data and wrap it based on returntype.
            match crate::pywrapper::py_any_to_data(result_any) {
                Ok(data_value) => {
                    let mut rust_do = DataObject::new();
                    rust_do.put_string("status", "ok");

                    let key_to_use = if returntype == "String" {
                        // If returntype is "String", ensure the data_value is actually a DString.
                        // If not, it's a type mismatch from Python, but we still use "msg".
                        // A more robust system might error here if type(data_value) != DString.
                        "msg"
                    } else {
                        // For "JSONObject", "JSONArray", "Integer", "Boolean", "Float", "Any", "File"
                        "data"
                    };
                    rust_do.set_property(key_to_use, data_value);
                    Ok(rust_do)
                }
                Err(e) => {
                    // This error means crate::pywrapper::py_any_to_data failed.
                    Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(format!(
                        "Failed to convert Python return type '{}' to ndata::Data: {}",
                        result_any.get_type().name()?, e
                    )))
                }
            }
        })
    }
}

fn compute_module_name(base_path: &str, file_path: &str) -> String {
    Path::new(file_path)
        .strip_prefix(base_path)
        .unwrap_or_else(|_| Path::new(file_path))
        .with_extension("")
        .to_string_lossy()
        .replace("/", ".")
        .replace("\\", ".")
}

pub fn dopy(lib: &str, id: &str, args: DataObject) -> DataObject {
    START.call_once(|| {
        let venv_path_str = DataStore::new().root
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("venv")
            .to_string_lossy()
            .to_string();

        let venv_option = if Path::new(&venv_path_str).exists() {
            Some(venv_path_str.as_str())
        } else {
            None
        };
        *PYENV.write().unwrap() = Some(PyEnv::new(venv_option));
    });

    let store = DataStore::new();
    let cmd_def = store.get_data(lib, id).get_object("data");
    let python_target_cmd_id = cmd_def.get_string("python"); // This is the ID of the command definition that has the python code/file info
    let actual_func_name = cmd_def.get_string("name"); // This is the function name within the python file

    // The PyCmd::get_path likely uses 'python_target_cmd_id' to find the .py file details.
    let py_file_path = PyCmd::get_path(lib, id); // Resolves to the .py file.

    // Get "returntype" and "params" from the command definition that holds the Python code details.
    let target_cmd_info = store.get_data(lib, &python_target_cmd_id).get_object("data");
    let returntype_str = target_cmd_info.get_string("returntype"); // Get the returntype
    let expected_params_array = target_cmd_info.get_array("params");

    let mut call_args = DataObject::new();
    for param_obj_data in expected_params_array.objects() {
        if let Data::DObject(param_obj_ref) = param_obj_data {
            let param_obj = DataObject::get(param_obj_ref);
            let key = param_obj.get_string("name");
            if let Ok(val) = args.try_get_property(&key) {
                call_args.set_property(&key, val);
            }
        }
    }

    let py_env_guard = PYENV.read().unwrap();
    let py_env = py_env_guard.as_ref().expect("PyEnv not initialized");

    // Pass returntype_str to execute_function
    match py_env.execute_function(&py_file_path, &actual_func_name, call_args, &returntype_str) {
        Ok(data_object) => data_object,
        Err(e) => {
            eprintln!("Error calling Python function '{}' in '{}': {}", actual_func_name, py_file_path, e);
            let mut err_obj = DataObject::new();
            err_obj.put_string("status", "error");
            err_obj.put_string("type", "PythonExecutionError");
            err_obj.put_string("message", &e.to_string());
            err_obj.put_string("python_function", &actual_func_name);
            err_obj.put_string("python_file", &py_file_path);
            err_obj
        }
    }
}
