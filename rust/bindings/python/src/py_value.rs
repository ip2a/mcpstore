use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};
use pyo3::IntoPyObjectExt;

pub fn to_py_object<T: serde::Serialize>(
    py: Python<'_>,
    value: &T,
    context: &str,
) -> PyResult<Py<PyAny>> {
    let value = serde_json::to_value(value).map_err(|err| {
        pyo3::exceptions::PyRuntimeError::new_err(format!("{context} serialization failed: {err}"))
    })?;
    serde_value_to_py(py, value)
}

fn serde_value_to_py(py: Python<'_>, value: serde_json::Value) -> PyResult<Py<PyAny>> {
    match value {
        serde_json::Value::Null => Ok(py.None()),
        serde_json::Value::Bool(value) => value.into_py_any(py),
        serde_json::Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                value.into_py_any(py)
            } else if let Some(value) = value.as_u64() {
                value.into_py_any(py)
            } else if let Some(value) = value.as_f64() {
                value.into_py_any(py)
            } else {
                Err(pyo3::exceptions::PyValueError::new_err(
                    "Unsupported serde number",
                ))
            }
        }
        serde_json::Value::String(value) => value.into_py_any(py),
        serde_json::Value::Array(values) => {
            let list = PyList::empty(py);
            for value in values {
                list.append(serde_value_to_py(py, value)?)?;
            }
            Ok(list.into_any().unbind())
        }
        serde_json::Value::Object(values) => {
            let dict = PyDict::new(py);
            for (key, value) in values {
                dict.set_item(key, serde_value_to_py(py, value)?)?;
            }
            Ok(dict.into_any().unbind())
        }
    }
}

pub fn py_to_serde_value(value: &Bound<'_, PyAny>, context: &str) -> PyResult<serde_json::Value> {
    if value.is_none() {
        return Ok(serde_json::Value::Null);
    }
    if let Ok(dict) = value.downcast::<PyDict>() {
        let mut object = serde_json::Map::new();
        for (key, value) in dict.iter() {
            let key = key.extract::<String>().map_err(|err| {
                pyo3::exceptions::PyValueError::new_err(format!(
                    "{context} dict key must be str: {err}"
                ))
            })?;
            object.insert(key, py_to_serde_value(&value, context)?);
        }
        return Ok(serde_json::Value::Object(object));
    }
    if let Ok(list) = value.downcast::<PyList>() {
        let mut values = Vec::with_capacity(list.len());
        for value in list.iter() {
            values.push(py_to_serde_value(&value, context)?);
        }
        return Ok(serde_json::Value::Array(values));
    }
    if let Ok(tuple) = value.downcast::<PyTuple>() {
        let mut values = Vec::with_capacity(tuple.len());
        for value in tuple.iter() {
            values.push(py_to_serde_value(&value, context)?);
        }
        return Ok(serde_json::Value::Array(values));
    }
    if let Ok(value) = value.extract::<bool>() {
        return Ok(serde_json::Value::Bool(value));
    }
    if let Ok(value) = value.extract::<i64>() {
        return Ok(serde_json::Value::Number(value.into()));
    }
    if let Ok(value) = value.extract::<u64>() {
        return Ok(serde_json::Value::Number(value.into()));
    }
    if let Ok(value) = value.extract::<f64>() {
        return serde_json::Number::from_f64(value)
            .map(serde_json::Value::Number)
            .ok_or_else(|| {
                pyo3::exceptions::PyValueError::new_err(format!(
                    "{context} float value must be finite"
                ))
            });
    }
    if let Ok(value) = value.extract::<String>() {
        return Ok(serde_json::Value::String(value));
    }
    Err(pyo3::exceptions::PyTypeError::new_err(format!(
        "{context} contains unsupported Python value: {}",
        value.get_type().name()?
    )))
}
