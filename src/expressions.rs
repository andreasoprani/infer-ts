//! Polars expression plugin functions.
//!
//! These `#[polars_expr]` functions are discovered by symbol name in the `.so`
//! and called via `register_plugin_function()` from Python.

use polars::prelude::*;
use pyo3_polars::derive::polars_expr;
use pyo3_polars::export::polars_core::utils::CustomIterTools;
use serde::Deserialize;

use crate::formats::{Format, UnixPrecision};
use crate::inference::{self, FeedResult, InferState};

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
    let name = series.name();

    // Format hint: skip inference and parse directly with Polars' as_datetime.
    if let Some(ref f) = kwargs.format {
        if let Some(precision) = parse_unix_marker(f) {
            return unix_to_datetime(ca, precision, name);
        }
        let ambiguous =
            StringChunked::from_slice(PlSmallStr::from_static("ambiguous"), &["raise"]);
        let parsed = ca.as_datetime(
            Some(f),
            TimeUnit::Microseconds,
            false, // use_cache
            false, // tz_aware
            None,  // tz
            &ambiguous,
        )?;
        return Ok(parsed.into_series().with_name(name.clone()));
    }

    // No format hint: single-pass inference + parse.
    single_pass_infer_and_parse(ca, kwargs.exhaustive, kwargs.raise_on_multiple, name)
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

// ─── Single-pass inference + parse ──────────────────────────────────────────

/// Infer format and parse to microseconds in a single pass over the Arrow column.
///
/// # Algorithm
/// Values are fed to [`InferState`] one at a time. While format is unsettled,
/// values are buffered as trimmed strings. On early exit (`FeedResult::Done`),
/// buffered values are parsed immediately and subsequent values are parsed
/// directly — no second iteration over the column is needed. In exhaustive or
/// ambiguous cases all values are buffered and parsed once format is finalised.
fn single_pass_infer_and_parse(
    ca: &StringChunked,
    exhaustive: bool,
    raise_on_multiple: bool,
    name: &PlSmallStr,
) -> PolarsResult<Series> {
    let mut state = InferState::new(exhaustive);
    // Accumulates values (trimmed strings or None) seen before format is settled.
    let mut buffer: Vec<Option<String>> = Vec::new();
    let mut results: Vec<Option<i64>> = Vec::with_capacity(ca.len());
    let mut settled_fmt: Option<Format> = None;

    for opt_val in ca.into_iter() {
        if let Some(fmt) = settled_fmt {
            // Format is known — parse directly without touching the buffer.
            match opt_val {
                None => results.push(None),
                Some(v) => results.push(fmt.parse_to_us(v.trim())),
            }
        } else {
            match opt_val {
                None => buffer.push(None),
                Some(v) => {
                    let trimmed = v.trim();
                    match state.feed(trimmed) {
                        FeedResult::NoMatch => {
                            polars_bail!(ComputeError: "no timestamp format matches the values");
                        }
                        FeedResult::Done(fmt) => {
                            settled_fmt = Some(fmt);
                            // Parse all buffered values accumulated during inference.
                            for opt in buffer.drain(..) {
                                match opt {
                                    None => results.push(None),
                                    Some(s) => results.push(fmt.parse_to_us(&s)),
                                }
                            }
                            // Parse the current value.
                            results.push(fmt.parse_to_us(trimmed));
                        }
                        FeedResult::Continue => {
                            // Store the trimmed string so we can parse it once settled.
                            buffer.push(Some(trimmed.to_string()));
                        }
                    }
                }
            }
        }
    }

    // Handle the case where inference never completed via early exit.
    // This happens in exhaustive mode or when the format remains ambiguous.
    if settled_fmt.is_none() {
        let formats = state.finish();

        if formats.is_empty() {
            // All values were null — return an all-null datetime series.
            return Series::new_null(name.clone(), ca.len())
                .cast(&DataType::Datetime(TimeUnit::Microseconds, None));
        }

        if formats.len() > 1 && raise_on_multiple {
            let fmt_strs: Vec<String> = formats.iter().map(|f| f.polars_format()).collect();
            polars_bail!(ComputeError:
                "multiple timestamp formats match: {:?}. \
                 Pass raise_on_multiple=False to use the first match, \
                 or pass format= explicitly.",
                fmt_strs
            );
        }

        let fmt = formats[0];
        for opt in buffer.drain(..) {
            match opt {
                None => results.push(None),
                Some(s) => results.push(fmt.parse_to_us(&s)),
            }
        }
    }

    let int_ca: Int64Chunked = results.into_iter().collect();
    Ok(int_ca
        .into_datetime(TimeUnit::Microseconds, None)
        .into_series()
        .with_name(name.clone()))
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
