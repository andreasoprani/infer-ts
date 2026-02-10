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

mod formats;
mod inference;

/// Infer timestamp format(s) from a string column using compositional parsing.
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
///
/// Special cases:
/// - Empty list: no format matches all values, or column is all nulls/empty
/// - Multiple formats: ambiguous input (e.g., US vs EU date format)
///
/// Example (Python):
/// ```python
/// import infer_ts
///
/// # Default (non-exhaustive): returns as soon as unique format found
/// infer_ts.infer_format(["2024-01-15T10:30:00", "2024-06-20T08:00:00"])
/// # ['%Y-%m-%dT%H:%M:%S']
///
/// # Exhaustive: checks all values, returns all compatible formats
/// infer_ts.infer_format(["01/02/2024", "03/04/2024"], exhaustive=True)
/// # ['%d/%m/%Y', '%m/%d/%Y']  (both US and EU are compatible)
///
/// # No match returns empty list
/// infer_ts.infer_format(["not a timestamp"])
/// # []
/// ```
#[pyfunction]
#[pyo3(signature = (values, exhaustive=false))]
fn infer_format(values: Vec<Option<String>>, exhaustive: bool) -> Vec<String> {
    let refs: Vec<Option<&str>> = values.iter().map(|s| s.as_deref()).collect();
    let formats = inference::infer(&refs, exhaustive);
    formats.iter().map(|f| f.polars_format()).collect()
}

#[pymodule]
fn _infer_ts(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(infer_format, m)?)?;
    m.add("__version__", "0.1.0")?;
    Ok(())
}
