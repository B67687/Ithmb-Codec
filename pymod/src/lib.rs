//! Python bindings for ithmb-core via PyO3/maturin.
//!
//! Exposes three functions:
//! - `decode_ithmb` — decode a single .ithmb file
//! - `open_ithmb` — decode a `PhotoDB` container or bare .ithmb
//! - `list_profiles` — list all 54 known profiles

extern crate ithmb_core as _;
use std::sync::atomic::AtomicBool;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;

use ithmb_core as codec;

// ---------------------------------------------------------------------------
// Error conversion
// ---------------------------------------------------------------------------

/// Convert an ithmb-core `DecodeError` into a Python exception.
fn decode_error_to_py(err: &codec::DecodeError) -> PyErr {
    match err {
        codec::DecodeError::Io(_) | codec::DecodeError::Canceled(_) => PyRuntimeError::new_err(err.to_string()),
        _ => PyValueError::new_err(err.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Helper: build a Python dict from DecodedImage
// ---------------------------------------------------------------------------

/// Convert a `DecodedImage` into a Python dict with keys:
/// "width", "height", "data" (`PyBytes` of BGRA pixels), "format", "rotation".
fn decoded_image_to_dict<'py>(
    py: Python<'py>,
    img: &'py codec::DecodedImage,
) -> PyResult<Bound<'py, pyo3::types::PyDict>> {
    let dict = pyo3::types::PyDict::new(py);
    dict.set_item("width", img.width)?;
    dict.set_item("height", img.height)?;
    dict.set_item("data", PyBytes::new(py, &img.data))?;
    dict.set_item("format", "BGRA")?;
    dict.set_item("rotation", 0i32)?;
    Ok(dict)
}

// ---------------------------------------------------------------------------
// Public Python functions
// ---------------------------------------------------------------------------

/// Decode a single .ithmb file from raw bytes.
///
/// Args:
///     data: Raw .ithmb file bytes.
///     canceled: Optional cancellation flag (default: false).
///
/// Returns:
///     A dict with "width", "height", "data" (BGRA `PyBytes`), "format", "rotation".
///
/// Raises:
///     `ValueError`: If the data cannot be decoded.
///     `RuntimeError`: If an I/O or cancellation error occurs.
#[pyfunction]
#[pyo3(signature = (data, canceled=None))]
fn decode_ithmb(py: Python<'_>, data: &[u8], canceled: Option<bool>) -> PyResult<Py<PyAny>> {
    let flag = AtomicBool::new(canceled.unwrap_or(false));
    let img = codec::decode_ithmb(data, &flag).map_err(|e| decode_error_to_py(&e))?;
    let dict = decoded_image_to_dict(py, &img)?;
    Ok(dict.into())
}

/// Decode a `PhotoDB` container file or bare .ithmb from raw bytes.
///
/// Args:
///     data: Raw file bytes (`PhotoDB` or bare .ithmb).
///
/// Returns:
///     A list of dicts, each with "width", "height", "data" (BGRA `PyBytes`),
///     "format", "rotation".
///
/// Raises:
///     `ValueError`: If the data cannot be decoded.
///     `RuntimeError`: If an I/O or cancellation error occurs.
#[pyfunction]
fn open_ithmb(py: Python<'_>, data: &[u8]) -> PyResult<Vec<Py<PyAny>>> {
    let flag = AtomicBool::new(false);
    let images = codec::open_ithmb(data, &flag, None).map_err(|e| decode_error_to_py(&e))?;
    images
        .into_iter()
        .map(|img| {
            let dict = decoded_image_to_dict(py, &img)?;
            Ok(dict.into())
        })
        .collect()
}

/// List all known decoding profiles.
///
/// Returns:
///     A list of dicts, each with "name" (prefix string), "width", "height",
///     and "encoding" (e.g. `Rgb565`, `Rgb555`, `Yuv422`, etc.).
#[pyfunction]
fn list_profiles(py: Python<'_>) -> Vec<Py<PyAny>> {
    let profiles = codec::profile::built_in_profiles();
    profiles
        .into_iter()
        .map(|p| {
            let dict = pyo3::types::PyDict::new(py);
            let _ = dict.set_item("name", p.prefix.to_string());
            let _ = dict.set_item("width", p.width);
            let _ = dict.set_item("height", p.height);
            let _ = dict.set_item("encoding", format!("{:?}", p.encoding));
            dict.into()
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Module definition
// ---------------------------------------------------------------------------

/// ithmb-core — a Python wrapper for the ithmb-codec Rust library.
#[pymodule(name = "ithmb_core")]
fn ithmb_core_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(decode_ithmb, m)?)?;
    m.add_function(wrap_pyfunction!(open_ithmb, m)?)?;
    m.add_function(wrap_pyfunction!(list_profiles, m)?)?;
    Ok(())
}
