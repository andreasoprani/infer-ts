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

// ─── output type funcs ──────────────────────────────────────────────────────

fn to_datetime_output_us(_: &[Field]) -> PolarsResult<Field> {
    Ok(Field::new(
        PlSmallStr::from_static("datetime"),
        DataType::Datetime(TimeUnit::Microseconds, None),
    ))
}

fn to_datetime_output_ms(_: &[Field]) -> PolarsResult<Field> {
    Ok(Field::new(
        PlSmallStr::from_static("datetime"),
        DataType::Datetime(TimeUnit::Milliseconds, None),
    ))
}

fn to_datetime_output_ns(_: &[Field]) -> PolarsResult<Field> {
    Ok(Field::new(
        PlSmallStr::from_static("datetime"),
        DataType::Datetime(TimeUnit::Nanoseconds, None),
    ))
}

// ─── to_datetime_expr_{us,ms,ns} ────────────────────────────────────────────

#[polars_expr(output_type_func=to_datetime_output_us)]
fn to_datetime_expr_us(inputs: &[Series], kwargs: ToDatetimeKwargs) -> PolarsResult<Series> {
    to_datetime_impl(inputs, kwargs, TimeUnit::Microseconds)
}

#[polars_expr(output_type_func=to_datetime_output_ms)]
fn to_datetime_expr_ms(inputs: &[Series], kwargs: ToDatetimeKwargs) -> PolarsResult<Series> {
    to_datetime_impl(inputs, kwargs, TimeUnit::Milliseconds)
}

#[polars_expr(output_type_func=to_datetime_output_ns)]
fn to_datetime_expr_ns(inputs: &[Series], kwargs: ToDatetimeKwargs) -> PolarsResult<Series> {
    to_datetime_impl(inputs, kwargs, TimeUnit::Nanoseconds)
}

fn to_datetime_impl(
    inputs: &[Series],
    kwargs: ToDatetimeKwargs,
    time_unit: TimeUnit,
) -> PolarsResult<Series> {
    let series = &inputs[0];
    let ca = series.str()?;
    let name = series.name();

    let formats = match inference::infer(ca, kwargs.exhaustive) {
        inference::InferResult::NoData => {
            // All values are null — return a null Datetime series.
            return Series::new_null(name.clone(), ca.len())
                .cast(&DataType::Datetime(time_unit, None));
        }
        inference::InferResult::NoMatch => {
            polars_bail!(ComputeError:
                "no timestamp format matched the input values. \
                 Ensure all non-null values share a single consistent format."
            );
        }
        inference::InferResult::Formats(fmts) => fmts,
    };

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
        return unix_to_datetime(ca, uf.precision, time_unit, name);
    }

    let polars_fmt = fmt.polars_format();
    let ambiguous = StringChunked::from_slice(PlSmallStr::from_static("ambiguous"), &["raise"]);
    let parsed = ca.as_datetime(
        Some(&polars_fmt),
        time_unit,
        true,  // use_cache — key to matching native performance
        false, // tz_aware
        None,  // tz
        &ambiguous,
    )?;
    Ok(parsed.into_series().with_name(name.clone()))
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn unix_to_datetime(
    ca: &StringChunked,
    precision: UnixPrecision,
    target: TimeUnit,
    name: &PlSmallStr,
) -> PolarsResult<Series> {
    let int_ca: Int64Chunked = ca
        .into_iter()
        .map(|opt| opt.and_then(|s| s.trim().parse::<i64>().ok()))
        .collect_trusted();

    let scaled = scale_unix(int_ca, precision, target);

    Ok(scaled
        .into_datetime(target, None)
        .into_series()
        .with_name(name.clone()))
}

/// Scale a unix integer series from `from` precision to `target` time unit.
///
/// Both are expressed as powers of 10 relative to seconds, so the conversion
/// is a single integer multiply or divide — no intermediate cast needed.
fn scale_unix(ca: Int64Chunked, from: UnixPrecision, target: TimeUnit) -> Int64Chunked {
    let from_exp: i32 = match from {
        UnixPrecision::Seconds => 0,
        UnixPrecision::Milliseconds => 3,
        UnixPrecision::Microseconds => 6,
        UnixPrecision::Nanoseconds => 9,
    };
    let target_exp: i32 = match target {
        TimeUnit::Milliseconds => 3,
        TimeUnit::Microseconds => 6,
        TimeUnit::Nanoseconds => 9,
    };
    let diff = target_exp - from_exp;
    if diff == 0 {
        ca
    } else if diff > 0 {
        let factor = 10i64.pow(diff as u32);
        ca.into_iter()
            .map(|opt| opt.and_then(|v| v.checked_mul(factor)))
            .collect_trusted()
    } else {
        ca / 10i64.pow((-diff) as u32)
    }
}
