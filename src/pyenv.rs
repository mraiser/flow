// pyenv.rs

use ndata::dataobject::DataObject;
use ndata::data::Data;
use std::sync::{Once, RwLock};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use std::ffi::CString;
use pyo3::types::{PyDict, PyList, PyModule};
use std::path::Path;
use crate::DataStore;
use crate::pycmd::{PyCall, PyCmd, PY_SUPPORT};
use crate::pywrapper::NDataObjectPy;


static START: Once = Once::new();
static PYENV: RwLock<Option<PyEnv>> = RwLock::new(None);
// The error-report helpers. A PyOnceLock, not a lazy import: concurrent
// callers would otherwise see the module half-initialized.
static SUPPORT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

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
        //std::env::set_var("PYTHONHOME", "");
        //std::env::set_var("PYTHONPATH", "");
        std::env::set_var("PATH", format!("{}/bin:{}", venv_path, std::env::var("PATH").unwrap_or_default()));
        std::env::set_var("VIRTUAL_ENV", venv_path);
    }

    /// Runs one Python command in the embedded interpreter. Returns the
    /// converted return value, or the error report (exception, location in
    /// the command body, traceback) - see `pycmd` for the envelope.
    pub fn execute(&self, call: &PyCall) -> Result<Data, String> {
        Python::attach(|py| {
            self.execute_inner(py, call).map_err(|e| format_error(py, &e, call))
        })
    }

    fn execute_inner(&self, py: Python<'_>, call: &PyCall) -> PyResult<Data> {
        let base_path = call.base_dir.display().to_string();
        let file_path = call.file.display().to_string();
        {
            let sys_module = py.import("sys")?;
            // --- VOODO PATH BS ---
            if let Ok(venv_path) = std::env::var("VIRTUAL_ENV") {
                // 1. Synchronize the sys.path from the real CLI to handle Nix store paths
                let python_exe = format!("{}/bin/python", venv_path);
                if let Ok(output) = std::process::Command::new(&python_exe)
                    .arg("-c")
                    .arg("import sys, json; print(json.dumps(sys.path))")
                    .output()
                {
                    if output.status.success() {
                        if let Ok(path_json) = String::from_utf8(output.stdout) {
                            if let Ok(code) = CString::new(format!("import sys, json; sys.path = json.loads('{}')", path_json.trim())) {
                                let _ = py.run(&code, None, None);
                            }
                        }
                    }
                }

                // 2. MUST explicitly call site.addsitedir to trigger .pth files for editable installs (pip install -e)
                let py_version = py.version_info();
                let site_packages = format!(
                    "{}/lib/python{}.{}/site-packages",
                    venv_path, py_version.major, py_version.minor
                );

                if let Ok(site) = py.import("site") {
                    let _ = site.call_method1("addsitedir", (&site_packages,));
                }
            }
            // --------------------------------
            let binding = sys_module.getattr("path")?;
            let sys_path: &Bound<'_, PyList> = binding.cast()?;

            // Still insert base_path for your generated pycmd files
            if !sys_path.contains(&base_path)? {
                sys_path.insert(0, &base_path)?;
            }

            if let Ok(cwd) = std::env::current_dir() {
                let cwd_str = cwd.to_string_lossy().to_string();
                if !sys_path.contains(&cwd_str)? {
                    sys_path.insert(0, &cwd_str)?;
                }
            }

            if let Some(file_dir) = Path::new(&file_path).parent() {
                if let Some(file_dir_str) = file_dir.to_str() {
                    if !sys_path.contains(file_dir_str)? {
                        sys_path.insert(0, file_dir_str)?;
                    }
                }
            }

        }

        let module = py.import(call.module.as_str())?;
        let function = module.getattr(call.func.as_str())?;

        let kwargs = PyDict::new(py);
        for (key, data_value) in call.args.objects() {
            kwargs.set_item(key, crate::pywrapper::data_to_py_any(py, data_value.clone())?)?;
        }
        let result = function.call((), Some(&kwargs))?;

        // FLAT: the command returns the whole response object itself.
        if call.returntype == "FLAT"
            && result.extract::<PyRef<NDataObjectPy>>().is_err()
            && result.cast::<PyDict>().is_err()
        {
            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "a FLAT command must return a dict or NDataObject, got {}",
                crate::pywrapper::type_name(&result)
            )));
        }
        crate::pywrapper::py_any_to_data(&result).map_err(|e| {
            let msg = format!("return value of {} could not be converted: {}", call.label, e.value(py));
            if e.is_instance_of::<pyo3::exceptions::PyOverflowError>(py) {
                pyo3::exceptions::PyOverflowError::new_err(msg)
            } else {
                pyo3::exceptions::PyTypeError::new_err(msg)
            }
        })
    }
}

/// The error report for a failed call, built by `PY_SUPPORT.format_error`.
fn format_error(py: Python<'_>, err: &PyErr, call: &PyCall) -> String {
    let report = (|| -> PyResult<String> {
        let support = SUPPORT
            .get_or_try_init(py, || {
                let code = CString::new(PY_SUPPORT).expect("PY_SUPPORT has no NUL bytes");
                PyModule::from_code(py, &code, c"flowlang_pysupport.py", c"flowlang_pysupport")
                    .map(|m| m.unbind())
            })?
            .bind(py);
        // Before 3.12 the traceback may be held by the PyErr only.
        let value = err.value(py);
        if let Some(tb) = err.traceback(py) {
            let _ = value.setattr("__traceback__", tb);
        }
        support
            .getattr("format_error")?
            .call1((value, call.file.display().to_string(), call.func.as_str(), call.label.as_str()))?
            .extract::<String>()
    })();
    report.unwrap_or_else(|_| format!("{} ({})", err, call.label))
}

fn init_once() {
    START.call_once(|| {
        let venv_path_str = DataStore::new().root.canonicalize().unwrap();
        eprintln!("Checking for DataStore.root at: {:?}", venv_path_str);

        let venv_path_str = venv_path_str
            .parent()
            .unwrap()
            .join("venv")
            .to_string_lossy()
            .to_string();

        eprintln!("Checking for venv at: {}", venv_path_str);

        let venv_option = if Path::new(&venv_path_str).exists() {
            Some(venv_path_str.as_str())
        } else {
            None
        };
        *PYENV.write().unwrap() = Some(PyEnv::new(venv_option));
    });
}

/// Runs a prepared Python command in the embedded interpreter.
pub fn call(call: &PyCall) -> Result<Data, String> {
    init_once();
    let py_env_guard = PYENV.read().unwrap();
    let py_env = py_env_guard.as_ref().expect("PyEnv not initialized");
    py_env.execute(call)
}

/// Runs a Python command by library and id. Returns the same envelope a Rust
/// command returns: `{"a": value}`, or `{"a": {"status": "err", "msg"}}`.
pub fn dopy(lib: &str, id: &str, args: DataObject) -> DataObject {
    PyCmd::new(lib, id).execute(args).expect("PyCmd::execute is infallible")
}
