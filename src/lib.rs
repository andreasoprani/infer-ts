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
use pyo3_polars::PolarsAllocator;
use pyo3_polars::PySeries;

mod expressions;
mod formats;
mod inference;

#[global_allocator]
static ALLOC: PolarsAllocator = PolarsAllocator::new();

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
    Ok(inference::infer(ca, exhaustive)
        .into_formats()
        .iter()
        .map(|f| f.polars_format())
        .collect())
}

/// Infer timestamp format(s) from any Python iterable of strings (streaming).
///
/// Pulls values lazily from the iterator — only reads as many values as needed
/// for early-exit (when `exhaustive=False`).  This avoids materialising the
/// entire column in memory on the Python side.
///
/// Args:
///     iter: Any Python iterable yielding `str | None`.
///     exhaustive: If `True`, consume the full iterator.
///
/// Returns:
///     A list of Polars-compatible format strings.
#[pyfunction]
#[pyo3(signature = (iter, exhaustive=false))]
fn infer_format_iter(iter: &Bound<'_, PyAny>, exhaustive: bool) -> PyResult<Vec<String>> {
    let mut state = inference::InferState::new(exhaustive);

    for item in iter.try_iter()? {
        let item = item?;
        let opt_s: Option<String> = if item.is_none() {
            None
        } else {
            Some(item.extract()?)
        };
        match state.feed(opt_s.as_deref()) {
            inference::FeedResult::NoMatch => return Ok(vec![]),
            inference::FeedResult::Done(fmt) => return Ok(vec![fmt.polars_format()]),
            inference::FeedResult::Continue => {}
        }
    }

    Ok(state
        .finish()
        .into_formats()
        .iter()
        .map(|f| f.polars_format())
        .collect())
}

#[pymodule]
fn _infer_ts(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(infer_format_series, m)?)?;
    m.add_function(wrap_pyfunction!(infer_format_iter, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
