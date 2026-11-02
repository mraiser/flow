// src/pywrapper.rs

use pyo3::prelude::*;
use pyo3::create_exception;
use pyo3::types::{PyBool, PyByteArray, PyBytes, PyDict, PyFloat, PyInt, PyList, PySlice, PyString, PyTuple};
use pyo3::IntoPyObjectExt;

// Import the ndata types we will be wrapping.
// Corrected to use `ndata::` prefix as it's an external crate dependency.
use ndata::dataobject::{self as ndata_dataobject_module, DataObject}; // Aliased module
use ndata::dataarray::{self as ndata_dataarray_module, DataArray};   // Aliased module
use ndata::databytes::{self as ndata_databytes_module, DataBytes}; // Removed DataStream
use ndata::data::Data;
use crate::pycmd::Value;


// --- Python Exception Definition ---
create_exception!(ndata_py, NDataPyError, pyo3::exceptions::PyException);
create_exception!(
    flowlang,
    CommandError,
    pyo3::exceptions::PyException,
    "A command run by flowlang.call failed: it raised or panicked, or returned {\"status\": \"err\", ...}. str(e) is its message."
);

// --- PyO3 Wrapper Structs ---

/// A Python wrapper for ndata::dataobject::DataObject.
#[pyclass(name = "NDataObject")]
#[derive(Clone)]
pub struct NDataObjectPy(pub DataObject); // Made field pub

/// A Python wrapper for ndata::dataarray::DataArray.
#[pyclass(name = "NDataArray")]
#[derive(Clone)]
pub struct NDataArrayPy(pub DataArray); // Made field pub

/// A Python wrapper for ndata::databytes::DataBytes.
#[pyclass(name = "NDataBytes")]
#[derive(Clone)]
pub struct NDataBytesPy(pub DataBytes); // Made field pub


// --- Helper for Python-style index normalization ---
fn normalize_index(index: isize, len: usize) -> PyResult<usize> {
    let i = if index < 0 {
        index + len as isize
    } else {
        index
    };
    if i < 0 || i >= len as isize {
        Err(pyo3::exceptions::PyIndexError::new_err("index out of range"))
    } else {
        Ok(i as usize)
    }
}


// --- Conversion Helpers & Error Mapping ---

/// Converts any supported Python object into an `ndata::Data` enum variant.
///
/// The mapping is exact and lossless, or it is an error - never a silent
/// change of type or value:
///
/// | Python                                   | ndata      |
/// |------------------------------------------|------------|
/// | `NDataObject` / `NDataArray` / `NDataBytes` | the same handle (no copy) |
/// | `None`                                   | `DNull`    |
/// | `bool`                                   | `DBoolean` |
/// | `int` (incl. `IntEnum`, numpy ints)      | `DInt`; outside i64 -> `OverflowError` |
/// | `float` (incl. numpy floats, `Fraction`) | `DFloat`   |
/// | `str` (incl. `StrEnum`)                  | `DString`  |
/// | `bytes`, `bytearray`                     | `DBytes`   |
/// | `dict` with `str` keys                   | `DObject`  |
/// | `list`, `tuple`                          | `DArray`   |
///
/// Anything else (`set`, `Decimal`, `datetime`, plain `Enum`, arbitrary
/// objects) raises `TypeError` naming the type and an explicit conversion.
/// In particular a list of small ints is a list, not bytes, and a big int is
/// an error, not a float.
pub(crate) fn py_any_to_value(py_any: &Bound<'_, PyAny>) -> PyResult<Value> {
    if let Ok(obj) = py_any.extract::<PyRef<NDataObjectPy>>() {
        return Ok(Value::Object(obj.0.clone()));
    }
    if let Ok(arr) = py_any.extract::<PyRef<NDataArrayPy>>() {
        return Ok(Value::Array(arr.0.clone()));
    }
    if let Ok(bytes_obj) = py_any.extract::<PyRef<NDataBytesPy>>() {
        return Ok(Value::Bytes(bytes_obj.0.clone()));
    }
    if py_any.is_none() { return Ok(Value::Scalar(Data::DNull)); }
    // bool before int: bool is a subclass of int.
    if py_any.is_instance_of::<PyBool>() { return Ok(Value::Scalar(Data::DBoolean(py_any.extract::<bool>()?))); }
    if py_any.is_instance_of::<PyInt>() { return int_to_data(py_any).map(Value::Scalar); }
    if py_any.is_instance_of::<PyFloat>() { return Ok(Value::Scalar(Data::DFloat(py_any.extract::<f64>()?))); }
    if py_any.is_instance_of::<PyString>() { return Ok(Value::Scalar(Data::DString(py_any.extract::<String>()?))); }
    if let Ok(py_bytes) = py_any.cast::<PyBytes>() {
        return Ok(Value::Bytes(DataBytes::from_bytes(&py_bytes.as_bytes().to_vec())));
    }
    if let Ok(py_bytearray) = py_any.cast::<PyByteArray>() {
        return Ok(Value::Bytes(DataBytes::from_bytes(&py_bytearray.to_vec())));
    }
    if let Ok(dict) = py_any.cast::<PyDict>() {
        let mut obj = DataObject::new();
        for (key, value) in dict.iter() {
            let key_str: String = match key.cast::<PyString>() {
                Ok(k) => k.to_str()?.to_string(),
                Err(_) => return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                    "dict keys must be str to convert to ndata, got {} key {}",
                    type_name(&key), key.repr()?
                ))),
            };
            py_any_to_value(&value)?.put(&mut obj, &key_str);
        }
        return Ok(Value::Object(obj));
    }
    if py_any.is_instance_of::<PyList>() || py_any.is_instance_of::<PyTuple>() {
        let mut arr = DataArray::new();
        for item in py_any.try_iter()? {
            py_any_to_value(&item?)?.push(&mut arr);
        }
        return Ok(Value::Array(arr));
    }
    // Numeric types that are not int/float subclasses (numpy scalars,
    // fractions.Fraction) register with the numbers ABCs. Decimal registers
    // only as numbers.Number, so it falls through to the TypeError below
    // rather than being rounded to a float.
    let numbers = py_any.py().import("numbers")?;
    if py_any.is_instance(&numbers.getattr("Integral")?)? {
        let as_int = py_any.py().import("operator")?.call_method1("index", (py_any,))?;
        return int_to_data(&as_int).map(Value::Scalar);
    }
    if py_any.is_instance(&numbers.getattr("Real")?)? {
        return Ok(Value::Scalar(Data::DFloat(py_any.extract::<f64>()?)));
    }
    let type_name = type_name(py_any);
    let hint = match type_name.as_str() {
        "set" | "frozenset" => " - convert it explicitly, e.g. sorted(x) or list(x)",
        "Decimal" => " - convert it explicitly with str(x) (exact) or float(x) (rounded)",
        "datetime" | "date" | "time" => " - convert it explicitly, e.g. x.isoformat()",
        _ => " - convert it to str, int, float, bool, None, bytes, dict, list or tuple",
    };
    Err(pyo3::exceptions::PyTypeError::new_err(format!(
        "cannot convert Python {} to ndata{}", type_name, hint
    )))
}

/// The plain type name (`list`, `Decimal`), as Python's own messages use it.
pub(crate) fn type_name(obj: &Bound<'_, PyAny>) -> String {
    obj.get_type()
        .getattr("__name__")
        .and_then(|n| n.extract::<String>())
        .unwrap_or_else(|_| "object".to_string())
}

/// An int that fits in i64, else OverflowError (never a lossy float).
fn int_to_data(py_int: &Bound<'_, PyAny>) -> PyResult<Data> {
    match py_int.extract::<i64>() {
        Ok(i) => Ok(Data::DInt(i)),
        Err(_) => Err(pyo3::exceptions::PyOverflowError::new_err(format!(
            "int {} does not fit in a 64-bit signed integer (ndata Integer); convert it explicitly, e.g. str(n)",
            py_int.repr()?
        ))),
    }
}

/// Converts an owned value into a Python object. A container's handle moves
/// into its Python wrapper.
pub(crate) fn value_to_py(py: Python<'_>, value: Value) -> PyResult<Py<PyAny>> {
    match value {
        Value::Object(o) => NDataObjectPy(o).into_py_any(py),
        Value::Array(a) => NDataArrayPy(a).into_py_any(py),
        Value::Bytes(b) => NDataBytesPy(b).into_py_any(py),
        Value::Scalar(Data::DString(s)) => s.into_py_any(py),
        Value::Scalar(Data::DInt(i)) => i.into_py_any(py),
        Value::Scalar(Data::DFloat(f)) => f.into_py_any(py),
        Value::Scalar(Data::DBoolean(b)) => b.into_py_any(py),
        Value::Scalar(_) => Ok(py.None()),
    }
}

/// Converts a `Data` read from a live container (an ndata getter's result).
pub(crate) fn data_to_py_any(py: Python<'_>, data: Data) -> PyResult<Py<PyAny>> {
    value_to_py(py, Value::from_data(data))
}

/// Maps `ndata::dataobject::NDataError` to a Python exception.
pub(crate) fn to_py_err_object(err: ndata_dataobject_module::NDataError) -> PyErr {
    match err {
        ndata_dataobject_module::NDataError::KeyNotFound(key) => pyo3::exceptions::PyKeyError::new_err(key),
        ndata_dataobject_module::NDataError::WrongDataType { key, expected, found } => {
            pyo3::exceptions::PyTypeError::new_err(format!(
                "For key '{}', expected type '{}' but found '{}'",
                key, expected, found
            ))
        }
    }
}

/// Maps `ndata::dataarray::NDataError` to a Python exception.
pub(crate) fn to_py_err_array(err: ndata_dataarray_module::NDataError) -> PyErr {
    match err {
        ndata_dataarray_module::NDataError::IndexOutOfBounds { index, len } => {
            pyo3::exceptions::PyIndexError::new_err(format!(
                "Index {} is out of bounds for array of length {}",
                index, len
            ))
        }
        ndata_dataarray_module::NDataError::WrongDataType { index, expected, found } => {
            pyo3::exceptions::PyTypeError::new_err(format!(
                "At index {}, expected type '{}' but found '{}'",
                index, expected, found
            ))
        }
        ndata_dataarray_module::NDataError::InvalidArrayRef => {
            NDataPyError::new_err("This NDataArray handle is invalid (data may have been GC'd).")
        }
        ndata_dataarray_module::NDataError::KeyNotFound(k) => pyo3::exceptions::PyKeyError::new_err(k), // Should be rare for array
    }
}

/// Maps `ndata::databytes::NDataError` to a Python exception.
pub(crate) fn to_py_err_bytes(err: ndata_databytes_module::NDataError) -> PyErr {
    match err {
        ndata_databytes_module::NDataError::InvalidBytesRef => {
            NDataPyError::new_err("This NDataBytes handle is invalid (data may have been GC'd).")
        }
        ndata_databytes_module::NDataError::StreamNotReadable => {
            NDataPyError::new_err("Stream is not open for reading.")
        }
        ndata_databytes_module::NDataError::StreamNotWritable => {
            NDataPyError::new_err("Stream is not open for writing.")
        }
    }
}


// --- NDataObject Python Methods ---
#[pymethods]
impl NDataObjectPy {
    #[new]
    fn new() -> Self { NDataObjectPy(DataObject::new()) }

    #[staticmethod]
    fn from_json(json_string: &str) -> PyResult<Self> {
        DataObject::try_from_string(json_string)
            .map(NDataObjectPy)
            .map_err(|e| NDataPyError::new_err(e.to_string()))
    }

    fn to_json(&self) -> String { self.0.to_string() }
    fn shallow_copy(&self) -> Self { NDataObjectPy(self.0.shallow_copy()) }
    fn deep_copy(&self) -> Self { NDataObjectPy(self.0.deep_copy()) }

    fn get_string(&self, key: &str) -> PyResult<String> { self.0.try_get_string(key).map_err(to_py_err_object) }
    fn get_int(&self, key: &str) -> PyResult<i64> { self.0.try_get_int(key).map_err(to_py_err_object) }
    fn get_float(&self, key: &str) -> PyResult<f64> { self.0.try_get_float(key).map_err(to_py_err_object) }
    fn get_boolean(&self, key: &str) -> PyResult<bool> { self.0.try_get_boolean(key).map_err(to_py_err_object) }
    fn get_object(&self, key: &str) -> PyResult<NDataObjectPy> { self.0.try_get_object(key).map(NDataObjectPy).map_err(to_py_err_object) }
    fn get_array(&self, key: &str) -> PyResult<NDataArrayPy> { self.0.try_get_array(key).map(NDataArrayPy).map_err(to_py_err_object) }
    fn get_bytes(&self, key: &str) -> PyResult<NDataBytesPy> { self.0.try_get_bytes(key).map(NDataBytesPy).map_err(to_py_err_object) }

    fn put_string(&self, key: &str, value: &str) { self.0.clone().put_string(key, value); }
    fn put_int(&self, key: &str, value: i64) { self.0.clone().put_int(key, value); }
    fn put_float(&self, key: &str, value: f64) { self.0.clone().put_float(key, value); }
    fn put_boolean(&self, key: &str, value: bool) { self.0.clone().put_boolean(key, value); }
    fn put_object(&self, key: &str, obj: &NDataObjectPy) { self.0.clone().put_object(key, obj.0.clone()); }
    fn put_array(&self, key: &str, arr: &NDataArrayPy) { self.0.clone().put_array(key, arr.0.clone()); }
    fn put_bytes(&self, key: &str, bytes: &NDataBytesPy) { self.0.clone().put_bytes(key, bytes.0.clone()); }
    fn put_null(&self, key: &str) { self.0.clone().put_null(key); }

    fn __str__(&self) -> String { self.0.to_string() }
    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!("NDataObject({})", self.to_dict(py)?.bind(py).repr()?))
    }
    fn __len__(&self) -> usize { self.0.get_keys().len() }
    fn __contains__(&self, key: &Bound<'_, PyAny>) -> bool {
        match key.extract::<String>() { Ok(k) => self.0.has(&k), Err(_) => false }
    }

    fn __getitem__(&self, key: &Bound<'_, PyAny>, py: Python) -> PyResult<Py<PyAny>> {
        let k = key.extract::<String>().map_err(|_| pyo3::exceptions::PyKeyError::new_err(key.clone().unbind()))?;
        let data = self.0.try_get_property(&k).map_err(to_py_err_object)?;
        data_to_py_any(py, data)
    }

    // ---- the dict protocol (collections.abc.MutableMapping) ----

    /// Iterates over a snapshot of the keys.
    fn __iter__(&self, py: Python<'_>) -> PyResult<Py<NDataPyIterator>> {
        let items = self.0.get_keys().into_iter().map(|k| k.into_py_any(py)).collect::<PyResult<Vec<_>>>()?;
        Py::new(py, NDataPyIterator { items, index: 0 })
    }

    /// Live views, as dict's: `collections.abc.KeysView(self)` etc.
    fn keys(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> { abc_view(slf.as_any(), "KeysView") }
    fn values(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> { abc_view(slf.as_any(), "ValuesView") }
    fn items(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> { abc_view(slf.as_any(), "ItemsView") }

    #[pyo3(signature = (key, default = None))]
    fn get(&self, py: Python<'_>, key: &Bound<'_, PyAny>, default: Option<Py<PyAny>>) -> PyResult<Py<PyAny>> {
        match key.extract::<String>().ok().and_then(|k| self.0.try_get_property(&k).ok()) {
            Some(data) => data_to_py_any(py, data),
            None => Ok(default.unwrap_or_else(|| py.None())),
        }
    }

    #[pyo3(signature = (key, *default))]
    fn pop(&self, py: Python<'_>, key: &Bound<'_, PyAny>, default: &Bound<'_, PyTuple>) -> PyResult<Py<PyAny>> {
        if default.len() > 1 {
            return Err(pyo3::exceptions::PyTypeError::new_err(format!("pop expected at most 2 arguments, got {}", default.len() + 1)));
        }
        if let Ok(k) = key.extract::<String>() {
            if let Ok(data) = self.0.try_get_property(&k) {
                let item = Value::from_data(data);
                self.0.clone().remove_property(&k);
                return value_to_py(py, item);
            }
        }
        match default.get_item(0) {
            Ok(d) => Ok(d.unbind()),
            Err(_) => Err(pyo3::exceptions::PyKeyError::new_err(key.clone().unbind())),
        }
    }

    fn popitem(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let k = match self.0.get_keys().pop() {
            Some(k) => k,
            None => return Err(pyo3::exceptions::PyKeyError::new_err("popitem(): NDataObject is empty")),
        };
        let item = Value::from_data(self.0.get_property(&k));
        self.0.clone().remove_property(&k);
        (k, value_to_py(py, item)?).into_py_any(py)
    }

    #[pyo3(signature = (key, default = None))]
    fn setdefault(&self, py: Python<'_>, key: &str, default: Option<&Bound<'_, PyAny>>) -> PyResult<Py<PyAny>> {
        if !self.0.has(key) {
            match default {
                Some(d) => py_any_to_value(d)?.put(&mut self.0.clone(), key),
                None => self.0.clone().put_null(key),
            }
        }
        data_to_py_any(py, self.0.get_property(key))
    }

    /// `update(mapping_or_pairs=(), **kwargs)`, as dict's.
    #[pyo3(signature = (other = None, **kwargs))]
    fn update(&self, other: Option<&Bound<'_, PyAny>>, kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        let mut target = self.0.clone();
        if let Some(other) = other {
            if other.hasattr("keys")? {
                for k in other.call_method0("keys")?.try_iter()? {
                    let k = k?;
                    let key: String = k.extract().map_err(|_| dict_key_error(&k))?;
                    py_any_to_value(&other.get_item(&k)?)?.put(&mut target, &key);
                }
            } else {
                for pair in other.try_iter()? {
                    let (k, v): (Bound<'_, PyAny>, Bound<'_, PyAny>) = pair?.extract()?;
                    let key: String = k.extract().map_err(|_| dict_key_error(&k))?;
                    py_any_to_value(&v)?.put(&mut target, &key);
                }
            }
        }
        if let Some(kw) = kwargs {
            for (k, v) in kw.iter() {
                py_any_to_value(&v)?.put(&mut target, &k.extract::<String>()?);
            }
        }
        Ok(())
    }

    fn clear(&self) {
        let mut target = self.0.clone();
        for k in self.0.get_keys() { target.remove_property(&k); }
    }

    /// A shallow copy: a new object sharing the nested containers, as dict.copy().
    fn copy(&self) -> Self { NDataObjectPy(self.0.shallow_copy()) }

    /// A deep copy as native Python values: dict, list, bytes, str, int, float,
    /// bool, None. Use it to hand data to code that requires a real dict.
    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> { to_native(py, Value::Object(self.0.clone())) }

    /// Equal to a dict, Mapping or NDataObject with equal keys and values.
    fn __eq__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let mine = self.to_dict(py)?;
        let theirs = if let Ok(o) = other.extract::<PyRef<NDataObjectPy>>() {
            o.to_dict(py)?.into_bound(py)
        } else if other.is_instance_of::<PyDict>() {
            other.clone()
        } else if other.is_instance(&py.import("collections.abc")?.getattr("Mapping")?)? {
            py.get_type::<PyDict>().call1((other,))?
        } else {
            return Ok(py.NotImplemented());
        };
        mine.bind(py).eq(theirs)?.into_py_any(py)
    }

    fn __ne__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let eq = self.__eq__(py, other)?;
        if eq.is(&py.NotImplemented()) { return Ok(eq); }
        (!eq.bind(py).is_truthy()?).into_py_any(py)
    }

    fn __setitem__(&self, key: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        py_any_to_value(value)?.put(&mut self.0.clone(), key);
        Ok(())
    }

    fn __delitem__(&self, key: &Bound<'_, PyAny>) -> PyResult<()> {
        match key.extract::<String>() {
            Ok(k) if self.0.has(&k) => { self.0.clone().remove_property(&k); Ok(()) }
            _ => Err(pyo3::exceptions::PyKeyError::new_err(key.clone().unbind())),
        }
    }
}

fn dict_key_error(k: &Bound<'_, PyAny>) -> PyErr {
    let r = k.repr().map(|r| r.to_string()).unwrap_or_default();
    pyo3::exceptions::PyTypeError::new_err(format!("NDataObject keys must be str, got {} key {}", type_name(k), r))
}

fn abc_view(mapping: &Bound<'_, PyAny>, view: &str) -> PyResult<Py<PyAny>> {
    Ok(mapping.py().import("collections.abc")?.getattr(view)?.call1((mapping,))?.unbind())
}

/// Deep conversion to native Python values (dict, list, bytes, scalars).
fn to_native(py: Python<'_>, value: Value) -> PyResult<Py<PyAny>> {
    match value {
        Value::Object(o) => {
            let d = PyDict::new(py);
            for k in o.get_keys() {
                d.set_item(&k, to_native(py, Value::from_data(o.get_property(&k)))?)?;
            }
            d.into_py_any(py)
        }
        Value::Array(a) => {
            let items = a.objects().into_iter().map(|d| to_native(py, Value::from_data(d))).collect::<PyResult<Vec<_>>>()?;
            PyList::new(py, items)?.into_py_any(py)
        }
        Value::Bytes(b) => {
            let data = b.try_get_data().map_err(to_py_err_bytes)?;
            PyBytes::new(py, &data).into_py_any(py)
        }
        scalar => value_to_py(py, scalar),
    }
}

/// `ndata_py.to_py(x)`: a deep native copy of an ndata value - and of any
/// dict/list/tuple containing them - for code that needs real dicts and
/// lists, e.g. `json.dumps(x, default=ndata_py.to_py)`. Anything else is
/// returned as is.
#[pyfunction]
fn to_py(py: Python<'_>, x: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    if let Ok(o) = x.extract::<PyRef<NDataObjectPy>>() { return to_native(py, Value::Object(o.0.clone())); }
    if let Ok(a) = x.extract::<PyRef<NDataArrayPy>>() { return to_native(py, Value::Array(a.0.clone())); }
    if let Ok(b) = x.extract::<PyRef<NDataBytesPy>>() { return to_native(py, Value::Bytes(b.0.clone())); }
    if let Ok(d) = x.cast::<PyDict>() {
        let out = PyDict::new(py);
        for (k, v) in d.iter() { out.set_item(k, to_py(py, &v)?)?; }
        return out.into_py_any(py);
    }
    if x.is_instance_of::<PyList>() || x.is_instance_of::<PyTuple>() {
        let items = x.try_iter()?.map(|i| to_py(py, &i?)).collect::<PyResult<Vec<_>>>()?;
        if x.is_instance_of::<PyTuple>() { return PyTuple::new(py, items)?.into_py_any(py); }
        return PyList::new(py, items)?.into_py_any(py);
    }
    Ok(x.clone().unbind())
}

// --- Custom Iterator for NDataArrayPy ---
#[pyclass]
struct NDataPyIterator {
    items: Vec<Py<PyAny>>,
    index: usize,
}

#[pymethods]
impl NDataPyIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> { slf }
    fn __next__(&mut self, py: Python) -> Option<Py<PyAny>> {
        if self.index < self.items.len() {
            let item = self.items[self.index].clone_ref(py);
            self.index += 1;
            Some(item)
        } else { None }
    }
}

// --- NDataArray Python Methods ---
#[pymethods]
impl NDataArrayPy {
    #[new]
    fn new() -> Self { NDataArrayPy(DataArray::new()) }

    #[staticmethod]
    fn from_json(json_string: &str) -> Self { NDataArrayPy(DataArray::from_string(json_string)) }

    fn to_json(&self) -> String { self.0.to_string() }
    fn shallow_copy(&self) -> Self { NDataArrayPy(self.0.shallow_copy()) }
    fn deep_copy(&self) -> Self { NDataArrayPy(self.0.deep_copy()) }

    fn append(&self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        py_any_to_value(value)?.push(&mut self.0.clone());
        Ok(())
    }

    fn extend(&self, iterable: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Ok(other_array) = iterable.extract::<PyRef<NDataArrayPy>>() {
            self.0.clone().join(other_array.0.clone());
            return Ok(());
        }
        for item_result in iterable.try_iter()? {
            self.append(&item_result?)?;
        }
        Ok(())
    }

    #[pyo3(signature = (index = -1))]
    fn pop(&self, index: isize, py: Python) -> PyResult<Py<PyAny>> {
        let len = self.0.len();
        if len == 0 { return Err(pyo3::exceptions::PyIndexError::new_err("pop from empty list")); }
        let py_idx = normalize_index(index, len)?; // Use normalize_index
        // Take a handle on the item, then remove it with remove_property,
        // which releases the array's reference. (pop_property hands that
        // reference back inside a bare Data; wrapping it with a new handle
        // leaked one reference per popped container.)
        let item = Value::from_data(self.0.get_property(py_idx));
        self.0.clone().remove_property(py_idx);
        value_to_py(py, item)
    }

    fn __str__(&self) -> String { self.0.to_string() }
    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!("NDataArray({})", self.to_list(py)?.bind(py).repr()?))
    }
    fn __len__(&self) -> usize { self.0.len() }

    // ---- the list protocol (collections.abc.MutableSequence) ----

    fn __contains__(&self, py: Python<'_>, x: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(self.position(py, x, 0, self.0.len())?.is_some())
    }

    #[pyo3(signature = (x, start = 0, stop = None))]
    fn index(&self, py: Python<'_>, x: &Bound<'_, PyAny>, start: isize, stop: Option<isize>) -> PyResult<usize> {
        let len = self.0.len() as isize;
        let clamp = |i: isize| (if i < 0 { i + len } else { i }).clamp(0, len) as usize;
        let (from, to) = (clamp(start), clamp(stop.unwrap_or(len)));
        self.position(py, x, from, to)?
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("value is not in NDataArray"))
    }

    fn count(&self, py: Python<'_>, x: &Bound<'_, PyAny>) -> PyResult<usize> {
        let mut n = 0;
        for d in self.0.objects() {
            if data_to_py_any(py, d)?.bind(py).eq(x)? { n += 1; }
        }
        Ok(n)
    }

    fn remove(&self, py: Python<'_>, x: &Bound<'_, PyAny>) -> PyResult<()> {
        match self.position(py, x, 0, self.0.len())? {
            Some(i) => { self.0.clone().remove_property(i); Ok(()) }
            None => Err(pyo3::exceptions::PyValueError::new_err("NDataArray.remove(x): x not in NDataArray")),
        }
    }

    /// Inserts before `index`, as list.insert (out-of-range indices clamp).
    fn insert(&self, index: isize, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let v = py_any_to_value(value)?;
        let len = self.0.len();
        let at = (if index < 0 { index + len as isize } else { index }).clamp(0, len as isize) as usize;
        let mut arr = self.0.clone();
        // ndata has no insert: lift the tail off (as handles), push, put it back.
        let tail: Vec<Value> = (at..len).map(|k| Value::from_data(arr.get_property(k))).collect();
        for _ in at..len { arr.remove_property(at); }
        v.push(&mut arr);
        for t in tail { t.push(&mut arr); }
        Ok(())
    }

    fn reverse(&self) {
        let mut arr = self.0.clone();
        let items: Vec<Value> = arr.objects().into_iter().map(Value::from_data).collect();
        while arr.len() > 0 { arr.remove_property(arr.len() - 1); }
        for v in items.into_iter().rev() { v.push(&mut arr); }
    }

    fn clear(&self) {
        let mut arr = self.0.clone();
        while arr.len() > 0 { arr.remove_property(arr.len() - 1); }
    }

    /// A shallow copy, as list.copy().
    fn copy(&self) -> Self { NDataArrayPy(self.0.shallow_copy()) }

    /// A deep copy as native Python values (list, dict, bytes, scalars).
    fn to_list(&self, py: Python<'_>) -> PyResult<Py<PyAny>> { to_native(py, Value::Array(self.0.clone())) }

    fn __iadd__(&self, other: &Bound<'_, PyAny>) -> PyResult<()> { self.extend(other) }

    /// Equal to a list or NDataArray with equal items.
    fn __eq__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let mine = self.to_list(py)?;
        let theirs = if let Ok(a) = other.extract::<PyRef<NDataArrayPy>>() {
            a.to_list(py)?.into_bound(py)
        } else if other.is_instance_of::<PyList>() {
            other.clone()
        } else {
            return Ok(py.NotImplemented());
        };
        mine.bind(py).eq(theirs)?.into_py_any(py)
    }

    fn __ne__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let eq = self.__eq__(py, other)?;
        if eq.is(&py.NotImplemented()) { return Ok(eq); }
        (!eq.bind(py).is_truthy()?).into_py_any(py)
    }

    fn __iter__(slf: PyRef<'_, Self>, py: Python) -> PyResult<Py<NDataPyIterator>> {
        let items_data = slf.0.objects();
        let items_py = items_data.into_iter().map(|d| data_to_py_any(py, d)).collect::<PyResult<Vec<_>>>()?;
        Py::new(py, NDataPyIterator { items: items_py, index: 0 })
    }

    /// Iterates over a snapshot, last item first.
    fn __reversed__(slf: PyRef<'_, Self>, py: Python) -> PyResult<Py<NDataPyIterator>> {
        let mut items = slf.0.objects().into_iter().map(|d| data_to_py_any(py, d)).collect::<PyResult<Vec<_>>>()?;
        items.reverse();
        Py::new(py, NDataPyIterator { items, index: 0 })
    }

    fn __getitem__(&self, index_or_slice: &Bound<'_, PyAny>, py: Python) -> PyResult<Py<PyAny>> {
        if let Ok(index) = index_or_slice.extract::<isize>() {
            let len = self.0.len();
            let py_idx = normalize_index(index, len)?; // Use normalize_index
            let data = self.0.try_get_property(py_idx).map_err(to_py_err_array)?;
            return data_to_py_any(py, data);
        }
        if let Ok(slice) = index_or_slice.cast::<PySlice>() {
             // PySlice::indices can return an error if step is 0
            // indices() rejects a zero step. Walk slicelength items from start
            // by step - a negative step included (a[::-1] was empty: the
            // step was cast to usize).
            let ind = slice.indices(self.0.len() as isize)?;
            let mut new_array = DataArray::new();
            for k in 0..ind.slicelength {
                let i = (ind.start + k as isize * ind.step) as usize;
                Value::from_data(self.0.get_property(i)).push(&mut new_array);
            }
            return NDataArrayPy(new_array).into_py_any(py);
        }
        Err(pyo3::exceptions::PyTypeError::new_err("NDataArray indices must be integers or slices"))
    }

    fn __setitem__(&self, index: isize, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let len = self.0.len();
        let py_idx = normalize_index(index, len)?; // Use normalize_index
        py_any_to_value(value)?.put_at(&mut self.0.clone(), py_idx);
        Ok(())
    }

    fn __delitem__(&self, index: isize) -> PyResult<()> {
        let len = self.0.len();
        let py_idx = normalize_index(index, len)?; // Use normalize_index
        self.0.clone().remove_property(py_idx); // remove_property in Rust panics on out of bounds
        Ok(())
    }
}

impl NDataArrayPy {
    /// The first index in from..to whose item == x.
    fn position(&self, py: Python<'_>, x: &Bound<'_, PyAny>, from: usize, to: usize) -> PyResult<Option<usize>> {
        for i in from..to.min(self.0.len()) {
            if data_to_py_any(py, self.0.get_property(i))?.bind(py).eq(x)? { return Ok(Some(i)); }
        }
        Ok(None)
    }
}

// --- NDataBytes Python Methods ---
#[pymethods]
impl NDataBytesPy {
    #[new]
    fn new() -> Self { NDataBytesPy(DataBytes::new()) }

    #[staticmethod]
    fn from_bytes(_py: Python, data: &Bound<'_, PyBytes>) -> Self { // _py is unused, marked with underscore
        NDataBytesPy(DataBytes::from_bytes(&data.as_bytes().to_vec()))
    }

    // Changed return type to Py<PyBytes>
    fn get_data<'py>(&self, py: Python<'py>) -> PyResult<Py<PyBytes>> {
        self.0.try_get_data()
            .map(|d| PyBytes::new(py, d.as_slice()).unbind())
            .map_err(to_py_err_bytes)
    }

    fn write(&mut self, data: &Bound<'_, PyBytes>) -> PyResult<()> { // Changed to &mut self for consistency if original try_write needed it
        self.0.clone().try_write(data.as_bytes()).map_err(to_py_err_bytes)
    }

    // Changed return type to Py<PyBytes>
    fn read(&mut self, n: usize, py: Python) -> PyResult<Py<PyBytes>> { // Changed to &mut self
        self.0.clone().try_read(n)
            .map(|d| PyBytes::new(py, d.as_slice()).unbind())
            .map_err(to_py_err_bytes)
    }

    fn set_data(&mut self, data: &Bound<'_, PyBytes>) -> PyResult<()> { // Changed to &mut self
        self.0.clone().try_set_data(&data.as_bytes().to_vec()).map_err(to_py_err_bytes)
    }

    fn current_len(&self) -> PyResult<usize> { self.0.try_current_len().map_err(to_py_err_bytes) }
    fn stream_len(&self) -> PyResult<usize> { self.0.try_stream_len().map_err(to_py_err_bytes) }
    fn set_stream_len(&mut self, len: usize) -> PyResult<()> { self.0.clone().try_set_stream_len(len).map_err(to_py_err_bytes) } // &mut self
    fn is_write_open(&self) -> PyResult<bool> { self.0.try_is_write_open().map_err(to_py_err_bytes) }
    fn is_read_open(&self) -> PyResult<bool> { self.0.try_is_read_open().map_err(to_py_err_bytes) }
    fn close_write(&mut self) -> PyResult<()> { self.0.clone().try_close_write().map_err(to_py_err_bytes) } // &mut self
    fn close_read(&mut self) -> PyResult<()> { self.0.clone().try_close_read().map_err(to_py_err_bytes) }   // &mut self
    fn get_mime_type(&self) -> PyResult<Option<String>> { self.0.try_get_mime_type().map_err(to_py_err_bytes) }
    fn set_mime_type(&mut self, mime_type: Option<String>) -> PyResult<()> { self.0.clone().try_set_mime_type(mime_type).map_err(to_py_err_bytes) } // &mut self

    fn to_hex_string(&self) -> String {
        if let Ok(data_vec) = self.0.try_get_data() {
            data_vec.iter().map(|b| format!("{:02X}", b)).collect::<Vec<String>>().join(" ")
        } else {
            String::from("Error: Invalid NDataBytes reference")
        }
    }

    fn deep_copy(&self) -> Self { NDataBytesPy(self.0.deep_copy()) }
    fn __repr__(&self) -> String {
        let len = self.0.try_current_len().unwrap_or(0);
        format!("<NDataBytes ref={} len={}>", self.0.data_ref, len)
    }
    fn __len__(&self) -> PyResult<usize> { self.current_len() }

    fn __bytes__<'py>(&self, py: Python<'py>) -> PyResult<Py<PyBytes>> { self.get_data(py) }

    /// Equal to bytes, bytearray or NDataBytes with the same content.
    fn __eq__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let theirs: Vec<u8> = if let Ok(b) = other.extract::<PyRef<NDataBytesPy>>() {
            b.0.try_get_data().map_err(to_py_err_bytes)?
        } else if let Ok(b) = other.cast::<PyBytes>() {
            b.as_bytes().to_vec()
        } else if let Ok(b) = other.cast::<PyByteArray>() {
            b.to_vec()
        } else {
            return Ok(py.NotImplemented());
        };
        (self.0.try_get_data().map_err(to_py_err_bytes)? == theirs).into_py_any(py)
    }

    fn __ne__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let eq = self.__eq__(py, other)?;
        if eq.is(&py.NotImplemented()) { return Ok(eq); }
        (!eq.bind(py).is_truthy()?).into_py_any(py)
    }
}


// --- Main Python Module Definition ---
// `init` initializes the ndata heaps, `gc` runs a collection - the same
// calls the old `#[pyfn]` versions made.
#[pyfunction(name = "init")]
fn ndata_init() {
    ndata::dataobject::DataObject::init();
    ndata::dataarray::DataArray::init();
    ndata::databytes::DataBytes::init();
}

#[pyfunction(name = "gc")]
fn ndata_gc() {
    ndata::dataobject::DataObject::gc();
    ndata::dataarray::DataArray::gc();
    ndata::databytes::DataBytes::gc();
}

#[pymodule]
pub(crate) fn ndata_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(ndata_init, m)?)?;
    m.add_function(wrap_pyfunction!(ndata_gc, m)?)?;
    m.add_function(wrap_pyfunction!(to_py, m)?)?;
    m.add("NDataPyError", m.py().get_type::<NDataPyError>())?;
    m.add_class::<NDataObjectPy>()?;
    m.add_class::<NDataArrayPy>()?;
    m.add_class::<NDataBytesPy>()?;

    Ok(())
}


// --- The `flowlang` module: calling other commands from Python ---

/// `flowlang.call(lib, ctl, cmd, /, **kwargs)` runs any command - Rust,
/// Python, flow, JavaScript or Java - and returns its value:
///
/// - a non-flow command's return value (its `a`);
/// - a flow's single output, or all of its outputs as an `NDataObject` when it
///   has several.
///
/// Raises `LookupError` for an unknown command, `TypeError` when an argument
/// cannot be cast to the parameter's declared type, and
/// `flowlang.CommandError` when the command fails: it raises or panics, or
/// returns an object whose `status` is `"err"` (the error envelope every
/// command wrapper uses). The interpreter lock is released while the command
/// runs, so other Python commands are not blocked by a slow callee.
#[pyfunction]
#[pyo3(signature = (lib, ctl, cmd, /, **kwargs))]
fn call(py: Python<'_>, lib: &str, ctl: &str, cmd: &str, kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<Py<PyAny>> {
    let args = DataObject::new();
    if let Some(kw) = kwargs {
        for (k, v) in kw.iter() {
            let k: String = k.extract()?;
            py_any_to_value(&v)
                .map_err(|e| pyo3::exceptions::PyTypeError::new_err(format!("argument {}: {}", k, e.value(py))))?
                .put(&mut args.clone(), &k);
        }
    }
    let (lib, ctl, cmd) = (lib.to_string(), ctl.to_string(), cmd.to_string());
    match py.detach(move || run_command(&lib, &ctl, &cmd, args)) {
        Ok(value) => value_to_py(py, value),
        Err(CallFailure::NotFound(m)) => Err(pyo3::exceptions::PyLookupError::new_err(m)),
        Err(CallFailure::BadArgs(m)) => Err(pyo3::exceptions::PyTypeError::new_err(m)),
        Err(CallFailure::Failed(m)) => Err(CommandError::new_err(m)),
    }
}

enum CallFailure {
    NotFound(String),
    BadArgs(String),
    Failed(String),
}

fn panic_text(p: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = p.downcast_ref::<&str>() { s.to_string() }
    else if let Some(s) = p.downcast_ref::<String>() { s.clone() }
    else { "unknown panic".to_string() }
}

/// Runs a command and returns its value as an owned `Value`.
fn run_command(lib: &str, ctl: &str, cmd: &str, args: DataObject) -> Result<Value, CallFailure> {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let label = format!("{}:{}:{}", lib, ctl, cmd);
    let command = catch_unwind(AssertUnwindSafe(|| crate::command::Command::lookup(lib, ctl, cmd)))
        .map_err(|p| CallFailure::NotFound(format!("no command {}: {}", label, panic_text(&p))))?;
    let a2 = args.clone();
    catch_unwind(AssertUnwindSafe(|| command.cast_params(a2))).map_err(|p| {
        CallFailure::BadArgs(format!("arguments for {} do not match its declared parameter types: {}", label, panic_text(&p)))
    })?;
    let result = match catch_unwind(AssertUnwindSafe(|| command.execute(args))) {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => return Err(CallFailure::Failed(format!("{} failed: {:?}", label, e))),
        Err(p) => return Err(CallFailure::Failed(format!("{} panicked: {}", label, panic_text(&p)))),
    };
    let value = if command.lang == "flow" {
        let keys = result.clone().keys();
        if keys.len() == 1 { Value::from_data(result.get_property(&keys[0])) } else { Value::Object(result) }
    } else if result.has("a") {
        Value::from_data(result.get_property("a"))
    } else {
        Value::Object(result)
    };
    if let Value::Object(o) = &value {
        if let Ok(Data::DString(status)) = o.try_get_property("status") {
            if status == "err" {
                let msg = match o.try_get_property("msg") {
                    Ok(Data::DString(m)) => m,
                    Ok(other) => format!("{:?}", other),
                    Err(_) => "(no msg)".to_string(),
                };
                return Err(CallFailure::Failed(format!("{}: {}", label, msg)));
            }
        }
    }
    Ok(value)
}

#[pymodule]
pub(crate) fn flowlang(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(call, m)?)?;
    m.add("CommandError", m.py().get_type::<CommandError>())?;
    Ok(())
}

/// Makes `ndata_py` and `flowlang` importable in the embedded interpreter.
/// Called once, when the interpreter is set up (pyenv).
pub(crate) fn register_modules(py: Python<'_>) -> PyResult<()> {
    let modules = py.import("sys")?.getattr("modules")?;
    let nd = pyo3::wrap_pymodule!(ndata_py)(py);
    // isinstance(o, Mapping) / isinstance(a, MutableSequence) hold, so code
    // that accepts any mapping or sequence accepts these.
    let abc = py.import("collections.abc")?;
    abc.getattr("MutableMapping")?.call_method1("register", (nd.bind(py).getattr("NDataObject")?,))?;
    abc.getattr("MutableSequence")?.call_method1("register", (nd.bind(py).getattr("NDataArray")?,))?;
    modules.set_item("ndata_py", nd)?;
    modules.set_item("flowlang", pyo3::wrap_pymodule!(flowlang)(py))?;
    Ok(())
}
