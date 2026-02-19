//! Polars expression plugin functions.
//!
//! These `#[polars_expr]` functions are discovered by symbol name in the `.so`
//! and called via `register_plugin_function()` from Python.

use polars::prelude::*;
use pyo3_polars::derive::polars_expr;
use pyo3_polars::export::polars_core::utils::CustomIterTools;
use serde::Deserialize;

use crate::formats::{Format, UnixPrecision};
use crate::inference;

// ─── kwargs ─────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ToDatetimeKwargs {
    exhaustive: bool,
    raise_on_multiple: bool,
}

#[derive(Deserialize)]
struct InferFormatKwargs {
    exhaustive: bool,
}

// ─── to_datetime_expr ───────────────────────────────────────────────────────

fn to_datetime_output(_: &[Field]) -> PolarsResult<Field> {
    Ok(Field::new(
        PlSmallStr::from_static("datetime"),
        DataType::Datetime(TimeUnit::Microseconds, None),
    ))
}

#[polars_expr(output_type_func=to_datetime_output)]
fn to_datetime_expr(inputs: &[Series], kwargs: ToDatetimeKwargs) -> PolarsResult<Series> {
    let series = &inputs[0];
    let ca = series.str()?;
    let name = series.name();

    // Infer then delegate to Polars as_datetime with cache.
    let formats = inference::infer(ca, kwargs.exhaustive);

    if formats.is_empty() {
        return Series::new_null(name.clone(), ca.len())
            .cast(&DataType::Datetime(TimeUnit::Microseconds, None));
    }

    if formats.len() > 1 && kwargs.raise_on_multiple {
        let fmt_strs: Vec<String> = formats.iter().map(|f| f.polars_format()).collect();
        polars_bail!(ComputeError:
            "multiple timestamp formats match: {:?}. \
             Pass raise_on_multiple=False to use the first match.",
            fmt_strs
        );
    }

    let fmt = formats[0];

    if let Format::Unix(uf) = fmt {
        return unix_to_datetime(ca, uf.precision, name);
    }

    let polars_fmt = fmt.polars_format();
    let ambiguous =
        StringChunked::from_slice(PlSmallStr::from_static("ambiguous"), &["raise"]);
    let parsed = ca.as_datetime(
        Some(&polars_fmt),
        TimeUnit::Microseconds,
        true,  // use_cache — key to matching native performance
        false, // tz_aware
        None,  // tz
        &ambiguous,
    )?;
    Ok(parsed.into_series().with_name(name.clone()))
}

// ─── infer_format_expr ──────────────────────────────────────────────────────

#[polars_expr(output_type=String)]
fn infer_format_expr(inputs: &[Series], kwargs: InferFormatKwargs) -> PolarsResult<Series> {
    let series = &inputs[0];
    let ca = series.str()?;

    let formats = inference::infer(ca, kwargs.exhaustive);

    let result = if formats.is_empty() {
        Series::new_null(series.name().clone(), 1).cast(&DataType::String)?
    } else {
        let fmt_str = formats[0].polars_format();
        Series::new(series.name().clone(), &[fmt_str])
    };

    Ok(result)
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn unix_to_datetime(
    ca: &StringChunked,
    precision: UnixPrecision,
    name: &PlSmallStr,
) -> PolarsResult<Series> {
    let (time_unit, multiplier): (TimeUnit, i64) = match precision {
        UnixPrecision::Seconds => (TimeUnit::Milliseconds, 1_000),
        UnixPrecision::Milliseconds => (TimeUnit::Milliseconds, 1),
        UnixPrecision::Microseconds => (TimeUnit::Microseconds, 1),
        UnixPrecision::Nanoseconds => (TimeUnit::Nanoseconds, 1),
    };

    let int_ca: Int64Chunked = ca
        .into_iter()
        .map(|opt| {
            opt.and_then(|s| {
                let trimmed = s.trim();
                trimmed.parse::<i64>().ok()
            })
        })
        .collect_trusted();

    let scaled = if multiplier != 1 {
        int_ca * multiplier
    } else {
        int_ca
    };

    let dt = scaled
        .into_datetime(time_unit, None)
        .into_series()
        .with_name(name.clone());

    // If target is microseconds, cast; otherwise keep native unit
    if time_unit != TimeUnit::Microseconds {
        dt.cast(&DataType::Datetime(TimeUnit::Microseconds, None))
    } else {
        Ok(dt)
    }
}
