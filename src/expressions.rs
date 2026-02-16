//! Polars expression plugin functions.
//!
//! These `#[polars_expr]` functions are discovered by symbol name in the `.so`
//! and called via `register_plugin_function()` from Python.

use polars::prelude::*;
use pyo3_polars::derive::polars_expr;
use pyo3_polars::export::polars_core::utils::CustomIterTools;
use serde::Deserialize;

use crate::formats::UnixPrecision;
use crate::inference;

// ─── kwargs ─────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ToDatetimeKwargs {
    format: Option<String>,
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

    let fmt = match kwargs.format {
        Some(f) => f,
        None => {
            let formats = inference::infer(ca, kwargs.exhaustive);

            if formats.is_empty() {
                if series.null_count() == series.len() {
                    return Series::new_null(series.name().clone(), series.len())
                        .cast(&DataType::Datetime(TimeUnit::Microseconds, None));
                }
                polars_bail!(ComputeError: "no timestamp format matches the values");
            }

            if formats.len() > 1 && kwargs.raise_on_multiple {
                let fmt_strs: Vec<String> = formats.iter().map(|f| f.polars_format()).collect();
                polars_bail!(ComputeError:
                    "multiple timestamp formats match: {:?}. \
                     Pass raise_on_multiple=False to use the first match, \
                     or pass format= explicitly.",
                    fmt_strs
                );
            }

            formats[0].polars_format()
        }
    };

    // Unix epoch handling
    if let Some(precision) = parse_unix_marker(&fmt) {
        return unix_to_datetime(ca, precision, series.name());
    }

    // Date-only or datetime string parsing
    let ambiguous = StringChunked::from_slice(PlSmallStr::from_static("ambiguous"), &["raise"]);
    let parsed = ca.as_datetime(
        Some(&fmt),
        TimeUnit::Microseconds,
        false, // use_cache
        false, // tz_aware
        None,  // tz
        &ambiguous,
    )?;
    Ok(parsed.into_series().with_name(series.name().clone()))
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

fn parse_unix_marker(fmt: &str) -> Option<UnixPrecision> {
    match fmt {
        "@unix_seconds" => Some(UnixPrecision::Seconds),
        "@unix_ms" => Some(UnixPrecision::Milliseconds),
        "@unix_us" => Some(UnixPrecision::Microseconds),
        "@unix_ns" => Some(UnixPrecision::Nanoseconds),
        _ => None,
    }
}

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
