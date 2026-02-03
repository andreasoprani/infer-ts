//! **infer-ts** – Infer timestamp formats from string columns using CSP
//! constraint elimination, exposed to Python via PyO3.
//!
//! Given a column of string values the library progressively eliminates
//! candidate timestamp formats that are inconsistent with each value.  It
//! returns as soon as a single format remains, or after every value has been
//! checked.
//!
//! The returned format string is compatible with Polars
//! `Expr.str.to_datetime(format=…)`.  Unix-epoch columns return a special
//! `@`-prefixed marker; see [`infer_format`] for details.

use pyo3::prelude::*;

mod formats;
mod inference;

/// Infer the timestamp format of a string column using constraint elimination.
///
/// Iterates through *values*, eliminating candidate formats that fail to
/// validate each entry.  Returns as soon as exactly one candidate remains.
///
/// Args:
///     values: A Python list of `str | None`.  `None` entries are skipped.
///
/// Returns:
///     A Polars-compatible format string, e.g. ``"%Y-%m-%dT%H:%M:%S"``.
///     For Unix epoch columns the return value is one of the special markers
///     ``"@unix_seconds"``, ``"@unix_ms"``, ``"@unix_us"``, ``"@unix_ns"``.
///     See the README for how to apply these with Polars.
///
/// Raises:
///     ValueError: No format matches all values, result is ambiguous, or the
///                 column contains only nulls.
///
/// Example (Python):
/// ```python
/// import inferts
/// inferts.infer_format(["2024-01-15T10:30:00", "2024-06-20T08:00:00"])
/// # '%Y-%m-%dT%H:%M:%S'
/// ```
#[pyfunction]
fn infer_format(values: Vec<Option<String>>) -> PyResult<String> {
    let refs: Vec<Option<&str>> = values.iter().map(|s| s.as_deref()).collect();
    let fmt = inference::infer(&refs)?;
    Ok(fmt.polars_format().to_string())
}

/// Return all supported timestamp formats as ``(name, polars_format)`` pairs.
///
/// Useful for introspection and documentation.
///
/// Example (Python):
/// ```python
/// import inferts
/// for name, fmt in inferts.supported_formats():
///     print(f"{name:45} → {fmt}")
/// ```
#[pyfunction]
fn supported_formats() -> Vec<(String, String)> {
    formats::Format::all()
        .iter()
        .map(|f| (f.name().to_string(), f.polars_format().to_string()))
        .collect()
}

#[pymodule]
fn infer_ts(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(infer_format, m)?)?;
    m.add_function(wrap_pyfunction!(supported_formats, m)?)?;
    m.add("__version__", "0.1.0")?;
    Ok(())
}
