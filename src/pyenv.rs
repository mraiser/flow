// pyenv.rs

use ndata::dataobject::DataObject;
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
        Python::attach(|py| {
            if let Err(e) = crate::pywrapper::register_modules(py) {
                eprintln!("Python: could not register the ndata_py and flowlang modules: {}", e);
            }
            if let Err(e) = support(py).and_then(|m| m.call_method0("install_api").map(|_| ())) {
                eprintln!("Python: could not install flowlang.api: {}", e);
            }
            if let Err(e) = configure_sys_path(py, venv_path) {
                eprintln!("Python: could not configure sys.path (venv {:?}): {}", venv_path, e);
            }
            // print(), sys.stderr and logging: complete lines, labelled with
            // the command, straight to the process's stdout (1) / stderr (2).
            if let Err(e) = support(py).and_then(|m| m.call_method1("install_output", (1, 2)).map(|_| ())) {
                eprintln!("Python: could not install the command output streams: {}", e);
            }
        });
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
    pub fn execute(&self, call: &PyCall) -> Result<DataObject, String> {
        Python::attach(|py| {
            self.execute_inner(py, call).map_err(|e| format_error(py, &e, call))
        })
    }

    fn execute_inner(&self, py: Python<'_>, call: &PyCall) -> PyResult<DataObject> {
        // The command's package root goes at the END of sys.path: a library
        // named like a stdlib or installed module (email, html, ...) must not
        // shadow that module for every other import. A library whose name is
        // taken gets a clear ImportError from import_command instead.
        let base_path = call.base_dir.display().to_string();
        let sys_path_any = py.import("sys")?.getattr("path")?;
        let sys_path: &Bound<'_, PyList> = sys_path_any.cast()?;
        if !sys_path.contains(&base_path)? {
            sys_path.append(&base_path)?;
        }

        let file_path = call.file.display().to_string();
        let module = support(py)?
            .getattr("import_command")?
            .call1((call.module.as_str(), base_path.as_str(), file_path.as_str()))?;
        let function = module.getattr(call.func.as_str())?;

        let kwargs = PyDict::new(py);
        for (key, data_value) in call.args.objects() {
            kwargs.set_item(key, crate::pywrapper::data_to_py_any(py, data_value.clone())?)?;
        }
        let support = support(py)?;
        let token = support.call_method1("begin_call", (call.label.as_str(),))?;
        let result = function.call((), Some(&kwargs));
        support.call_method1("end_call", (token,))?;
        let result = result?;

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
        let value = crate::pywrapper::py_any_to_value(&result).map_err(|e| {
            let msg = format!("return value of {} could not be converted: {}", call.label, e.value(py));
            if e.is_instance_of::<pyo3::exceptions::PyOverflowError>(py) {
                pyo3::exceptions::PyOverflowError::new_err(msg)
            } else {
                pyo3::exceptions::PyTypeError::new_err(msg)
            }
        })?;
        Ok(crate::pycmd::ok_envelope(value))
    }
}

/// The `PY_SUPPORT` helpers (import_command, format_error), loaded once.
fn support(py: Python<'_>) -> PyResult<&Bound<'_, PyModule>> {
    Ok(SUPPORT
        .get_or_try_init(py, || {
            let code = CString::new(PY_SUPPORT).expect("PY_SUPPORT has no NUL bytes");
            PyModule::from_code(py, &code, c"flowlang_pysupport.py", c"flowlang_pysupport")
                .map(|m| m.unbind())
        })?
        .bind(py))
}

/// Points the interpreter at the venv, once, at startup. Before, this ran on
/// EVERY call - including spawning the venv's python to read its sys.path -
/// which made a call with a venv about 70x slower than one without.
fn configure_sys_path(py: Python<'_>, venv: Option<&str>) -> PyResult<()> {
    let sys = py.import("sys")?;
    if let Some(venv) = venv {
        let venv = Path::new(venv);
        // 1. The venv interpreter's own sys.path, which can differ from the
        //    embedded one (e.g. Nix store paths).
        let exe = [venv.join("bin").join("python"), venv.join("Scripts").join("python.exe")]
            .into_iter()
            .find(|p| p.exists());
        if let Some(exe) = exe {
            match std::process::Command::new(&exe)
                .arg("-c")
                .arg("import sys, json; print(json.dumps(sys.path))")
                .output()
            {
                Ok(out) if out.status.success() => {
                    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    let paths = py.import("json")?.call_method1("loads", (text,))?;
                    sys.setattr("path", paths)?;
                }
                Ok(out) => eprintln!("Python: {} could not report its sys.path: {}", exe.display(), out.status),
                Err(e) => eprintln!("Python: could not run {}: {}", exe.display(), e),
            }
        }
        // 2. site.addsitedir processes the venv's .pth files, which is how
        //    editable installs (pip install -e) are found.
        let v = py.version_info();
        let site_packages = [
            venv.join("lib").join(format!("python{}.{}", v.major, v.minor)).join("site-packages"),
            venv.join("Lib").join("site-packages"),
        ];
        for sp in site_packages.iter().filter(|p| p.exists()) {
            py.import("site")?.call_method1("addsitedir", (sp.display().to_string(),))?;
        }
    }
    // No current-directory entry: `python -c` puts '' first in the list read
    // above, and commands must not import whatever sits in the server's
    // working directory.
    py.run(c"import sys\nsys.path[:] = [p for p in sys.path if p not in ('', '.')]", None, None)?;
    Ok(())
}

/// The error report for a failed call, built by `PY_SUPPORT.format_error`.
fn format_error(py: Python<'_>, err: &PyErr, call: &PyCall) -> String {
    let report = (|| -> PyResult<String> {
        let support = support(py)?;
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
pub fn call(call: &PyCall) -> Result<DataObject, String> {
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
