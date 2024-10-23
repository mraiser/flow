// pyenv.rs

use ndata::dataobject::DataObject;
use std::sync::{Once, RwLock};
use pyo3::prelude::*;
use pyo3::types::PyModule;
use std::path::Path;
use crate::DataStore;
use crate::pycmd::PyCmd;
use ndata::data::Data;
use pyo3::types::PyTuple;
use pyo3::types::PyDict;
use pyo3::types::PyList;
use ndata::databytes::DataBytes;
use ndata::dataarray::DataArray;
use std::collections::HashMap;
use serde_json::Value;
use crate::base64::Base64;
use ndata::data::Data::*;
use pyo3::types::PyBytes;

// Static variables for PyEnv singleton
static START: Once = Once::new();
static PYENV: RwLock<Option<PyEnv>> = RwLock::new(None);

pub struct PyEnv;

impl PyEnv {
    /// Create a new `PyEnv` and initialize the Python environment with venv support
    pub fn new(venv_path: Option<&str>) -> PyEnv {
        if let Some(path) = venv_path {
            if !Path::new(path).exists() {
                panic!("Virtual environment not found at {}", path);
            }
//            println!("Activating Python virtual environment: {:?}", path);
            PyEnv::activate_venv(path);
        }

        PyEnv
    }
    
    /// Method to activate a Python virtual environment
    fn activate_venv(venv_path: &str) {
        let venv_bin = format!("{}/bin/python", venv_path);

        // Ensure the Python interpreter from the venv is used
        std::env::set_var("PYTHONHOME", "");
        std::env::set_var("PYTHONPATH", "");
        std::env::set_var("PATH", format!("{}/bin:{}", venv_path, std::env::var("PATH").unwrap()));
        std::env::set_var("VIRTUAL_ENV", venv_path);

        // Confirm venv is active by checking the Python version
        let output = std::process::Command::new(&venv_bin)
            .arg("--version")
            .output()
            .expect("Failed to activate virtual environment");

        if !output.status.success() {
            panic!(
                "Failed to use Python from virtual environment: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        } else {
//            println!("Using Python: {}", String::from_utf8_lossy(&output.stdout));
        }
    }
    
    pub fn execute_function(&self, file_path: &str, name: &str, params: DataObject) -> PyResult<DataObject> {
        let base_path = "cmd/src";
        let module_name = compute_module_name(base_path, file_path);
//        println!("Attempting to execute function '{}' in module '{}'", name, module_name);

        Python::with_gil(|py| {
            // Add the directory containing the Python file to sys.path
            let sys_module = py.import("sys")?;
            let sys_path: &PyList = sys_module.getattr("path")?.downcast::<PyList>()?;

            if !sys_path.contains(base_path)? {
//                println!("Inserting base path into sys.path: {}", base_path);
                sys_path.insert(0, base_path)?;
            }

            // Get the directory of the Python file
            let file_dir = Path::new(file_path).parent().unwrap();
            let file_dir_str = file_dir.to_str().unwrap();

            if !sys_path.contains(file_dir_str)? {
//                println!("Inserting file directory into sys.path: {}", file_dir_str);
                sys_path.insert(0, file_dir_str)?;
            }

            // Import the module by its name
            let module = py.import(module_name.as_str())?;
//            println!("Module {:?} imported successfully!", module_name);

            // Prepare arguments as a dictionary for the Python function
            let kwargs = PyDict::new(py);
            for (key, data) in params.objects() {
                let py_value = data_to_py(data, py)?;
                kwargs.set_item(key, py_value)?;
            }

//            println!("PY KWARGS {:?}", kwargs);

            // Get the function from the module using getattr
            let function = module.getattr(name)?;

            // Call the Python function with keyword arguments
            let result: &PyAny = function.call((), Some(kwargs))?;

            // Convert the result back to DataObject
            Ok(py_any_to_data_object(result, py)?)
        })
    }
}

pub fn data_to_py(data: Data, py: Python) -> PyResult<PyObject> {
    match data {
        // Atomic data types can be converted directly
        Data::DInt(i) => Ok(i.to_object(py)),          // Convert i64 to PyObject
        Data::DFloat(f) => Ok(f.to_object(py)),        // Convert f64 to PyObject
        Data::DBoolean(b) => Ok(b.to_object(py)),      // Convert bool to PyObject
        Data::DString(s) => Ok(s.to_object(py)),       // Convert String to PyObject
        Data::DNull => Ok(py.None()),                  // Python's None
        
        // Complex data types need to be pulled from the Heap
        Data::DBytes(bytes_ref) => {
            let bytes = DataBytes::get(bytes_ref);  // Retrieve the actual bytes from the heap
            Ok(PyBytes::new(py, &bytes.get_data()).to_object(py))  // Convert Vec<u8> to PyBytes
        },
        Data::DArray(array_ref) => {
            let array = DataArray::get(array_ref);  // Retrieve the actual array from the heap

            // Convert each item in the array to a PyObject, collecting the results
            let py_objects: PyResult<Vec<PyObject>> = array.objects()
                .iter()
                .map(|item| data_to_py(item.clone(), py))
                .collect();

            // If successful, convert the Vec<PyObject> to a PyList
            Ok(PyList::new(py, py_objects?).to_object(py))
        },
        Data::DObject(object_ref) => {
            let obj = DataObject::get(object_ref);  // Retrieve the actual object from the heap
            let py_dict = PyDict::new(py);
            for (key, value) in obj.objects().iter() {
                py_dict.set_item(key, data_to_py(value.clone(), py)?)?;
            }
            Ok(py_dict.to_object(py))               // Convert DataObject to PyDict
        },

        // Handle other cases if needed, like streams, etc.
        _ => Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
            "Unsupported data type for conversion to Python.",
        )),
    }
}

fn py_any_to_data_object(py_any: &PyAny, py: Python) -> PyResult<DataObject> {
//    println!("converting {:?}", py_any);

    let mut data = DataObject::new();
    data.put_string("status", "ok"); // Set the status to "ok"

    // Determine the type of PyAny and convert it to the corresponding Data type
    if let Ok(s) = py_any.extract::<String>() {
        data.put_string("data", &s); // Use put_string for the data
    } else if let Ok(i) = py_any.extract::<i64>() {
        data.put_int("data", i); // Use put_int for the data
    } else if let Ok(f) = py_any.extract::<f64>() {
        data.put_float("data", f); // Use put_float for the data
    } else if let Ok(b) = py_any.extract::<bool>() {
        data.put_bool("data", b); // Use put_bool for the data
    } else if let Ok(bytes) = py_any.extract::<Vec<u8>>() {
        data.put_bytes("data", DataBytes::from_bytes(&bytes)); // Use put_bytes for the data
    } else if let Ok(py_dict) = py_any.extract::<&PyDict>() {
        // Convert PyDict to HashMap for serialization
        let mut map = DataObject::new();
        for (key, value) in py_dict.iter() {
            let key: String = key.extract()?;
            let value_do = py_any_to_data_object(value, py)?;
            map.set_property(&key, value_do.get_property("data"));
        }
        data.put_object("data", map); // Use put_object for the data
    } else if let Ok(py_list) = py_any.extract::<&PyList>() {
        // Convert PyList to Vec for serialization
        let mut vec = DataArray::new();
        for item in py_list.iter() {
            let value_do = py_any_to_data_object(item, py)?;
            vec.push_property(value_do.get_property("data"));
        }
        data.put_array("data", vec); // Use put_array for the data
    } else if py_any.is_none() {
        data.put_null("data"); // Use put_null for the data
    } else {
        // If none of the types match, return an error
        return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
            "Unsupported type for conversion to DataObject.",
        ));
    }

    Ok(data) // Return the DataObject
}

fn compute_module_name(base_path: &str, file_path: &str) -> String {
    let relative_path = Path::new(file_path)
        .strip_prefix(base_path)
        .expect("Base path not found in file path");
    
    let module_path = relative_path.with_extension(""); // Removes the .py extension
    let module_name = module_path.to_string_lossy().replace("/", ".").replace("\\", ".");
    
//    println!("module name is {}", module_name);
    module_name.to_string()
}

pub fn dopy(lib: &str, id: &str, args: DataObject) -> DataObject {
    // Initialize the PyEnv singleton if it doesn't exist yet
    START.call_once(|| {
        let venv = DataStore::new().root.parent().unwrap().join("venv");
        let venvstr = venv.display().to_string();
        let venv = match venv.exists() {
            true => Some(venvstr.as_ref()),
            _ => None,
        };
        *PYENV.write().unwrap() = Some(PyEnv::new(venv));
    });
    
    // Retrieve the DataStore
    let store = DataStore::new();
    let cmd = store.get_data(lib, id);
    let cmd = cmd.get_object("data");

    // Extract values
    let jsid = cmd.get_string("python");
    let name = cmd.get_string("name");

    // Get the path to the Python file containing the code
    let f = PyCmd::get_path(lib, id);
//    println!("PATH {:?}", f);

    let cmd = store.get_data(lib, &jsid);    
    let cmd = cmd.get_object("data");

    let code = cmd.get_string("python");
    let ctl = cmd.get_string("ctl");
    let params = cmd.get_array("params");

    // Prepare the arguments for the Python function
    let mut a = DataObject::new();
    for o in params.objects() {
        let key = o.object().get_string("name");
        let val = args.get_property(&key);
//        println!("Set {} to {}", key, Data::as_string(val.clone()));
        a.set_property(&key, val);
    }

    // Call the Python function
    let py_env = PYENV.read().unwrap();
    let result = py_env.as_ref()
        .expect("PyEnv not initialized")
        .execute_function(&f, &name, a);

    // Handle the result appropriately
    match result {
        Ok(data_object) => data_object,
        Err(e) => {
            eprintln!("Error calling Python function: {}", e);
            DataObject::new() // Return an empty DataObject or handle error as needed
        }
    }
}

