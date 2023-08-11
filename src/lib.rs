use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use serde::de::DeserializeSeed;

mod serde_pyobject;
use serde_pyobject::*;

#[pyfunction]
pub fn to_toml(py: Python, obj: PyObject) -> PyResult<PyObject> {
    let v = SerializePyObject {
        py,
        obj: obj.extract(py)?
    };
    let str = toml::to_string(&v)
        .map_err(|error| PyErr::new::<PyTypeError, _>(
            format!("TOML serialization error: {:?}", error.to_string())),
        )?;
    Ok(str.to_object(py))
}

#[pyfunction]
pub fn from_toml(py: Python, obj: PyObject) -> PyResult<PyObject> {
    let str: String = obj.extract(py)?;
    let deserializer = toml::Deserializer::new(&str);
    let seed = HyperJsonValue::new(py, &None, &None);
    let result = seed.deserialize(deserializer)
        .map_err(|error| PyErr::new::<PyTypeError, _>(
            format!("TOML deserialization error: {}", error.to_string())
        ))?;
    Ok(result)
}

#[pymodule]
fn pytomlrs<'py>(_py: Python<'py>, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(to_toml, m)?)?;
    m.add_function(wrap_pyfunction!(from_toml, m)?)?;
    Ok(())
}