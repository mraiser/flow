// src/pywrapper.rs

use pyo3::prelude::*;
use pyo3::create_exception;
use pyo3::types::{PyDict, PyList, PyBytes, PySlice}; // Corrected PySequence import, added PySlice

// Import the ndata types we will be wrapping.
// Corrected to use `ndata::` prefix as it's an external crate dependency.
use ndata::dataobject::{self as ndata_dataobject_module, DataObject}; // Aliased module
use ndata::dataarray::{self as ndata_dataarray_module, DataArray};   // Aliased module
use ndata::databytes::{self as ndata_databytes_module, DataBytes}; // Removed DataStream
use ndata::data::Data;


// --- Python Exception Definition ---
create_exception!(ndata_py, NDataPyError, pyo3::exceptions::PyException);

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
#[allow(clippy::needless_return)] // Allow explicit returns for clarity in match-like structure
pub(crate) fn py_any_to_data(py_any: &Bound<'_, PyAny>) -> PyResult<Data> {
    if let Ok(obj) = py_any.extract::<PyRef<NDataObjectPy>>() {
        return Ok(Data::DObject(obj.0.clone().data_ref));
    }
    if let Ok(arr) = py_any.extract::<PyRef<NDataArrayPy>>() {
        return Ok(Data::DArray(arr.0.clone().data_ref));
    }
    if let Ok(bytes_obj) = py_any.extract::<PyRef<NDataBytesPy>>() {
        return Ok(Data::DBytes(bytes_obj.0.clone().data_ref));
    }
    if py_any.is_none() { return Ok(Data::DNull); }
    if let Ok(s) = py_any.extract::<String>() { return Ok(Data::DString(s)); }
    if let Ok(b) = py_any.extract::<bool>() { return Ok(Data::DBoolean(b)); }
    if let Ok(i) = py_any.extract::<i64>() { return Ok(Data::DInt(i)); }
    if let Ok(f) = py_any.extract::<f64>() { return Ok(Data::DFloat(f)); }
    if let Ok(py_bytes) = py_any.downcast::<PyBytes>() {
        let rust_bytes: Vec<u8> = py_bytes.as_bytes().to_vec();
        return Ok(Data::DBytes(DataBytes::from_bytes(&rust_bytes).data_ref));
    }
    if let Ok(bytes_vec) = py_any.extract::<Vec<u8>>() {
         return Ok(Data::DBytes(DataBytes::from_bytes(&bytes_vec).data_ref));
    }
    if let Ok(dict) = py_any.downcast::<PyDict>() {
        let obj = DataObject::new();
        for (key, value) in dict.iter() {
            let key_str: String = key.extract()?;
            obj.clone().set_property(&key_str, py_any_to_data(&value)?);
        }
        return Ok(Data::DObject(obj.data_ref));
    }
    if let Ok(list) = py_any.downcast::<PyList>() {
        let arr = DataArray::new();
        for item in list.iter() {
            arr.clone().push_property(py_any_to_data(&item)?);
        }
        return Ok(Data::DArray(arr.data_ref));
    }
    Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(format!(
        "Unsupported Python type for conversion to ndata: {}",
        py_any.get_type().name()?
    )))
}

/// Converts an `ndata::Data` enum variant into a Python object.
pub(crate) fn data_to_py_any(py: Python, data: Data) -> PyObject {
    match data {
        Data::DObject(r) => NDataObjectPy(DataObject::get(r)).into_py(py),
        Data::DArray(r) => NDataArrayPy(DataArray::get(r)).into_py(py),
        Data::DBytes(r) => NDataBytesPy(DataBytes::get(r)).into_py(py),
        Data::DString(s) => s.into_py(py),
        Data::DInt(i) => i.into_py(py),
        Data::DFloat(f) => f.into_py(py),
        Data::DBoolean(b) => b.into_py(py),
        Data::DNull => py.None(),
    }
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
    fn keys(&self) -> Vec<String> { self.0.get_keys() }

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
    fn __repr__(&self) -> String { format!("<NDataObject ref={}>", self.0.data_ref) }
    fn __len__(&self) -> usize { self.0.get_keys().len() } // Assumes DataObject::get_keys().len() is efficient enough
    fn __contains__(&self, key: &str) -> bool { self.0.has(key) }

    fn __getitem__(&self, key: &str, py: Python) -> PyResult<PyObject> {
        self.0.try_get_property(key)
            .map(|data| data_to_py_any(py, data))
            .map_err(to_py_err_object)
    }

    fn __setitem__(&self, key: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let data = py_any_to_data(value)?;
        self.0.clone().set_property(key, data);
        Ok(())
    }

    fn __delitem__(&self, key: &str) -> PyResult<()> {
        if !self.0.has(key) {
            return Err(pyo3::exceptions::PyKeyError::new_err(key.to_string()));
        }
        self.0.clone().remove_property(key);
        Ok(())
    }
}

// --- Custom Iterator for NDataArrayPy ---
#[pyclass]
struct NDataPyIterator {
    items: Vec<PyObject>,
    index: usize,
}

#[pymethods]
impl NDataPyIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> { slf }
    fn __next__(&mut self, py: Python) -> Option<PyObject> {
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
        let data = py_any_to_data(value)?;
        self.0.clone().push_property(data);
        Ok(())
    }

    fn extend(&self, iterable: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Ok(other_array) = iterable.extract::<PyRef<NDataArrayPy>>() {
            self.0.clone().join(other_array.0.clone());
            return Ok(());
        }
        for item_result in iterable.iter()? {
            self.append(&item_result?)?;
        }
        Ok(())
    }

    #[pyo3(signature = (index = -1))]
    fn pop(&self, index: isize, py: Python) -> PyResult<PyObject> {
        let len = self.0.len();
        if len == 0 { return Err(pyo3::exceptions::PyIndexError::new_err("pop from empty list")); }
        let py_idx = normalize_index(index, len)?; // Use normalize_index
        // pop_property in Rust panics on out of bounds, ensure index is valid.
        // normalize_index already checks this.
        let data = self.0.clone().pop_property(py_idx);
        Ok(data_to_py_any(py, data))
    }

    fn __str__(&self) -> String { self.0.to_string() }
    fn __repr__(&self) -> String { format!("<NDataArray ref={}>", self.0.data_ref) }
    fn __len__(&self) -> usize { self.0.len() }

    fn __iter__(slf: PyRef<'_, Self>, py: Python) -> PyResult<Py<NDataPyIterator>> {
        let items_data = slf.0.objects();
        let items_py: Vec<PyObject> = items_data.into_iter().map(|d| data_to_py_any(py, d)).collect();
        Py::new(py, NDataPyIterator { items: items_py, index: 0 })
    }

    fn __getitem__(&self, index_or_slice: &Bound<'_, PyAny>, py: Python) -> PyResult<PyObject> {
        if let Ok(index) = index_or_slice.extract::<isize>() {
            let len = self.0.len();
            let py_idx = normalize_index(index, len)?; // Use normalize_index
            return self.0.try_get_property(py_idx)
                .map(|data| data_to_py_any(py, data))
                .map_err(to_py_err_array);
        }
        if let Ok(slice) = index_or_slice.downcast::<PySlice>() {
             // PySlice::indices can return an error if step is 0
            let indices = slice.indices(self.0.len() as i64)?; // Use isize for len
            let new_array = DataArray::new();
            // Ensure indices.start, indices.stop, indices.step are valid
            if indices.step == 0 {
                return Err(pyo3::exceptions::PyValueError::new_err("slice step cannot be zero"));
            }
            for i in (indices.start..indices.stop).step_by(indices.step as usize) {
                 // get_property panics on error, use try_get_property
                match self.0.try_get_property(i as usize) {
                    Ok(data) => new_array.clone().push_property(data),
                    Err(_) => return Err(pyo3::exceptions::PyIndexError::new_err("slice index out of bounds during construction"))
                }
            }
            return Ok(NDataArrayPy(new_array).into_py(py));
        }
        Err(pyo3::exceptions::PyTypeError::new_err("NDataArray indices must be integers or slices"))
    }

    fn __setitem__(&self, index: isize, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let len = self.0.len();
        let py_idx = normalize_index(index, len)?; // Use normalize_index
        let data = py_any_to_data(value)?;
        self.0.clone().set_property(py_idx, data);
        Ok(())
    }

    fn __delitem__(&self, index: isize) -> PyResult<()> {
        let len = self.0.len();
        let py_idx = normalize_index(index, len)?; // Use normalize_index
        self.0.clone().remove_property(py_idx); // remove_property in Rust panics on out of bounds
        Ok(())
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
            .map(|d| PyBytes::new_bound(py, d.as_slice()).into())
            .map_err(to_py_err_bytes)
    }

    fn write(&mut self, data: &Bound<'_, PyBytes>) -> PyResult<()> { // Changed to &mut self for consistency if original try_write needed it
        self.0.clone().try_write(data.as_bytes()).map_err(to_py_err_bytes)
    }

    // Changed return type to Py<PyBytes>
    fn read(&mut self, n: usize, py: Python) -> PyResult<Py<PyBytes>> { // Changed to &mut self
        self.0.clone().try_read(n)
            .map(|d| PyBytes::new_bound(py, d.as_slice()).into())
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
}


// --- Main Python Module Definition ---
#[pymodule]
fn ndata_py(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    #[pyfn(m)]
    fn init(_py: Python) {
        // Corrected to use ndata:: prefixes
        ndata::dataobject::DataObject::init();
        ndata::dataarray::DataArray::init();
        ndata::databytes::DataBytes::init();
    }

    #[pyfn(m)]
    fn gc(_py: Python) {
        ndata::dataobject::DataObject::gc();
        ndata::dataarray::DataArray::gc();
        ndata::databytes::DataBytes::gc();
    }

    m.add("NDataPyError", _py.get_type_bound::<NDataPyError>())?;
    m.add_class::<NDataObjectPy>()?;
    m.add_class::<NDataArrayPy>()?;
    m.add_class::<NDataBytesPy>()?;
    m.add_class::<NDataPyIterator>()?;

    Ok(())
}
