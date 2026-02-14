//! **infer-ts** – Infer timestamp formats from string columns using
//! compositional parsing, exposed to Python via PyO3.
//!
//! Given a column of string values the library parses the first non-null value
//! to seed candidate formats, then validates subsequent values against those
//! candidates. It returns as soon as a single format remains (unless
//! `exhaustive=True`), or after every value has been checked.
//!
//! The returned format strings are compatible with Polars
//! `Expr.str.to_datetime(format=…)`.  Unix-epoch columns return special
//! `@`-prefixed markers; see [`infer_format`] for details.

use pyo3::prelude::*;
use pyo3_polars::PySeries;

mod formats;
mod inference;

/// Infer timestamp format(s) from a Python list of strings.
///
/// Parses the first non-null value to seed candidate formats, then validates
/// subsequent values against those candidates.
///
/// Args:
///     values: A Python list of `str | None`.  `None` entries are skipped.
///     exhaustive: If `True`, process all values and return all compatible formats.
///                 If `False` (default), return as soon as only one format remains.
///
/// Returns:
///     A list of Polars-compatible format strings, e.g. `["%Y-%m-%dT%H:%M:%S"]`.
///     For Unix epoch columns the return values include special markers like
///     `"@unix_seconds"`, `"@unix_ms"`, `"@unix_us"`, `"@unix_ns"`.
///     See the README for how to apply these with Polars.
#[pyfunction]
#[pyo3(signature = (values, exhaustive=false))]
fn infer_format(values: Vec<Option<String>>, exhaustive: bool) -> Vec<String> {
    let refs: Vec<Option<&str>> = values.iter().map(|s| s.as_deref()).collect();
    let formats = inference::infer(refs, exhaustive);
    formats.iter().map(|f| f.polars_format()).collect()
}

/// Infer timestamp format(s) from a Polars Series (zero-copy).
///
/// Accepts a Polars `Series` of strings directly, avoiding the Python list
/// intermediate and Rust `Vec<String>` allocation. The underlying Arrow
/// `StringChunked` is iterated lazily with zero-copy access.
///
/// Args:
///     series: A Polars Series with String (Utf8) dtype.
///     exhaustive: If `True`, process all values and return all compatible formats.
///                 If `False` (default), return as soon as only one format remains.
///
/// Returns:
///     A list of Polars-compatible format strings.
///
/// Raises:
///     TypeError: If the Series is not of String dtype.
#[pyfunction]
#[pyo3(signature = (series, exhaustive=false))]
fn infer_format_series(series: PySeries, exhaustive: bool) -> PyResult<Vec<String>> {
    let series = series.0;
    let ca = series.str().map_err(|e| {
        PyErr::new::<pyo3::exceptions::PyTypeError, _>(format!("expected a String Series: {e}"))
    })?;
    let formats = inference::infer(ca, exhaustive);
    Ok(formats.iter().map(|f| f.polars_format()).collect())
}

#[pymodule]
fn _infer_ts(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(infer_format, m)?)?;
    m.add_function(wrap_pyfunction!(infer_format_series, m)?)?;
    m.add("__version__", "0.1.0")?;
    Ok(())
}
