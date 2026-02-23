//! Hybrid timestamp format inference: compositional parsing + constraint propagation.
//!
//! The algorithm uses lazy domain initialization:
//!
//! 1. **First non-null value** – parse it with [`Format::parse`] to discover all
//!    matching formats. This seeds the candidate set with only formats that actually
//!    match real data (no pre-enumeration needed).
//! 2. **Subsequent values** – retain only candidates whose [`Format::validates`]
//!    returns `true`. This narrows the set progressively.
//! 3. **Early exit** – if `exhaustive=false` and exactly one candidate remains,
//!    return it immediately.
//! 4. **Finalise** – return all surviving candidates.

use std::collections::HashSet;

use crate::formats::Format;

// ─── Inference State Machine ─────────────────────────────────────────────────

/// Incremental inference state that can be fed values one at a time.
///
/// This enables streaming use cases where values arrive lazily (e.g., from a
/// Python iterator) without materialising the entire column in memory.
pub struct InferState {
    candidates: Option<HashSet<Format>>,
    exhaustive: bool,
}

/// Result of feeding a value to [`InferState`].
pub(crate) enum FeedResult {
    /// Keep feeding more values.
    Continue,
    /// Inference is done — no formats match.
    NoMatch,
    /// Inference is done — early exit with a single format (non-exhaustive mode).
    Done(Format),
}

/// Result of completing inference over a sequence of values.
#[derive(Debug, PartialEq)]
pub(crate) enum InferResult {
    /// No non-null, non-whitespace values were seen — can't infer from empty data.
    NoData,
    /// Values were seen but no single format matched all of them.
    NoMatch,
    /// One or more formats survived (sorted for deterministic output).
    Formats(Vec<Format>),
}

impl InferResult {
    /// Extract the formats, returning an empty `Vec` for `NoData` and `NoMatch`.
    ///
    /// Useful for Python-facing functions that need a backward-compatible
    /// `Vec<Format>` (both empty-column and no-match become an empty list).
    pub fn into_formats(self) -> Vec<Format> {
        match self {
            Self::Formats(v) => v,
            _ => vec![],
        }
    }
}

impl InferState {
    pub fn new(exhaustive: bool) -> Self {
        Self {
            candidates: None,
            exhaustive,
        }
    }

    /// Feed a single value. `None` and whitespace-only values are skipped.
    pub fn feed(&mut self, value: Option<&str>) -> FeedResult {
        let value = match value {
            Some(v) => {
                let trimmed = v.trim();
                if trimmed.is_empty() {
                    return FeedResult::Continue;
                }
                trimmed
            }
            None => return FeedResult::Continue,
        };

        match &mut self.candidates {
            None => {
                self.candidates = Some(Format::parse(value).into_iter().collect());
            }
            Some(set) => {
                set.retain(|fmt| fmt.validates(value));
            }
        }

        if let Some(set) = &self.candidates {
            match set.len() {
                0 => return FeedResult::NoMatch,
                1 if !self.exhaustive => {
                    return FeedResult::Done(*set.iter().next().unwrap());
                }
                _ => {}
            }
        }

        FeedResult::Continue
    }

    /// Finalise inference and return the result.
    pub fn finish(self) -> InferResult {
        match self.candidates {
            None => InferResult::NoData,
            Some(set) if set.is_empty() => InferResult::NoMatch,
            Some(set) => {
                let mut result: Vec<Format> = set.into_iter().collect();
                result.sort();
                InferResult::Formats(result)
            }
        }
    }
}

// ─── Convenience Function ────────────────────────────────────────────────────

/// Run hybrid format inference over an iterator of (possibly null) string values.
///
/// `None` entries and whitespace-only strings are skipped (treated as nulls).
///
/// # Arguments
/// * `values` - Iterator of optional string values to infer format from.
///   Accepts `&[Option<&str>]`, `Vec<Option<&str>>`, or any `IntoIterator`
///   yielding `Option<&str>` (e.g., a Polars `StringChunked` iterator).
/// * `exhaustive` - If `true`, process all values and return all compatible formats.
///   If `false`, return as soon as only one format remains (early exit).
///
/// # Returns
/// An [`InferResult`] discriminating three outcomes:
/// - `Formats(v)` — one or more formats survived (sorted)
/// - `NoData` — no non-null values were seen; can't infer from empty data
/// - `NoMatch` — values were seen but no format matched all of them
pub fn infer<'a>(
    values: impl IntoIterator<Item = Option<&'a str>>,
    exhaustive: bool,
) -> InferResult {
    let mut state = InferState::new(exhaustive);

    for opt_value in values {
        match state.feed(opt_value) {
            FeedResult::NoMatch => return InferResult::NoMatch,
            FeedResult::Done(fmt) => return InferResult::Formats(vec![fmt]),
            FeedResult::Continue => {}
        }
    }

    state.finish()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::Format;

    // ── Helper ──────────────────────────────────────────────────────────────

    /// Wrap a slice of `&str` into `Vec<Option<&str>>` (all `Some`).
    fn vals<'a>(v: &[&'a str]) -> Vec<Option<&'a str>> {
        v.iter().map(|s| Some(*s)).collect()
    }

    // Format constructor helpers for concise test assertions
    fn dt(date: DateFmt, sep: Separator, time: TimeFmt, tz: Option<Timezone>) -> Format {
        Format::DateTime(DateTimeFormat {
            date,
            time: crate::formats::TimeComponent {
                separator: sep,
                format: time,
                timezone: tz,
                spaced_tz: false,
            },
        })
    }
    fn dt_spaced_tz(date: DateFmt, sep: Separator, time: TimeFmt, tz: Timezone) -> Format {
        Format::DateTime(DateTimeFormat {
            date,
            time: crate::formats::TimeComponent {
                separator: sep,
                format: time,
                timezone: Some(tz),
                spaced_tz: true,
            },
        })
    }
    fn date_only(date: DateFmt) -> Format {
        Format::Date(DateFormat { date })
    }
    fn unix(precision: UnixPrecision) -> Format {
        Format::Unix(UnixFormat { precision })
    }

    use crate::formats::{
        DateFmt::{self, *},
        DateFormat, DateTimeFormat,
        Separator::{self, *},
        TimeFmt::{self, *},
        Timezone::{self, *},
        UnixFormat,
        UnixPrecision::{self, *},
    };

    /// Assert inference result contains exactly these formats (order-independent).
    fn assert_infer(values: &[Option<&str>], exhaustive: bool, expected: &[Format]) {
        let result = match infer(values.iter().copied(), exhaustive) {
            InferResult::Formats(fmts) => fmts,
            other => panic!("expected Formats(..), got {:?}", other),
        };
        assert_eq!(
            result.len(),
            expected.len(),
            "expected {:?}, got {:?}",
            expected,
            result
        );
        for fmt in expected {
            assert!(
                result.contains(fmt),
                "expected {:?} in result, got {:?}",
                fmt,
                result
            );
        }
    }

    // ── Happy-path: every format family resolves correctly ─────────────────

    #[test]
    fn infer_iso8601_plain() {
        assert_infer(
            &vals(&["2024-01-15T10:30:00", "2024-06-20T08:00:00"]),
            false,
            &[dt(Iso, T, Hms, None)],
        );
    }

    #[test]
    fn infer_iso8601_utc() {
        assert_infer(
            &vals(&["2024-01-15T10:30:00Z"]),
            false,
            &[dt(Iso, T, Hms, Some(Utc))],
        );
    }

    #[test]
    fn infer_iso8601_offset() {
        assert_infer(
            &vals(&["2024-01-15T10:30:00+05:30"]),
            false,
            &[dt(Iso, T, Hms, Some(Offset))],
        );
    }

    #[test]
    fn infer_iso8601_frac() {
        assert_infer(
            &vals(&["2024-01-15T10:30:00.123"]),
            false,
            &[dt(Iso, T, HmsFrac, None)],
        );
    }

    #[test]
    fn infer_iso8601_frac_utc() {
        assert_infer(
            &vals(&["2024-01-15T10:30:00.123Z"]),
            false,
            &[dt(Iso, T, HmsFrac, Some(Utc))],
        );
    }

    #[test]
    fn infer_iso8601_frac_offset() {
        assert_infer(
            &vals(&["2024-01-15T10:30:00.123+05:30"]),
            false,
            &[dt(Iso, T, HmsFrac, Some(Offset))],
        );
    }

    #[test]
    fn infer_space_plain() {
        assert_infer(
            &vals(&["2024-01-15 10:30:00"]),
            false,
            &[dt(Iso, Space, Hms, None)],
        );
    }

    #[test]
    fn infer_space_frac() {
        assert_infer(
            &vals(&["2024-01-15 10:30:00.999999"]),
            false,
            &[dt(Iso, Space, HmsFrac, None)],
        );
    }

    #[test]
    fn infer_space_utc() {
        assert_infer(
            &vals(&["2024-01-15 10:30:00Z"]),
            false,
            &[dt(Iso, Space, Hms, Some(Utc))],
        );
    }

    #[test]
    fn infer_space_offset() {
        assert_infer(
            &vals(&["2024-01-15 10:30:00+05:30"]),
            false,
            &[dt(Iso, Space, Hms, Some(Offset))],
        );
    }

    #[test]
    fn infer_space_frac_utc() {
        assert_infer(
            &vals(&["2024-01-15 10:30:00.123Z"]),
            false,
            &[dt(Iso, Space, HmsFrac, Some(Utc))],
        );
    }

    #[test]
    fn infer_date_iso() {
        assert_infer(
            &vals(&["2024-01-15", "2024-06-20"]),
            false,
            &[date_only(Iso)],
        );
    }

    #[test]
    fn infer_slash_us_unambiguous() {
        assert_infer(
            &vals(&["01/15/2024", "06/20/2024"]),
            false,
            &[date_only(SlashUS)],
        );
    }

    #[test]
    fn infer_slash_eu_unambiguous() {
        assert_infer(
            &vals(&["15/01/2024", "20/06/2024"]),
            false,
            &[date_only(SlashEU)],
        );
    }

    #[test]
    fn infer_slash_us_datetime() {
        assert_infer(
            &vals(&["01/15/2024 10:30:00"]),
            false,
            &[dt(SlashUS, Space, Hms, None)],
        );
    }

    #[test]
    fn infer_slash_eu_datetime() {
        assert_infer(
            &vals(&["15/01/2024 10:30:00"]),
            false,
            &[dt(SlashEU, Space, Hms, None)],
        );
    }

    #[test]
    fn infer_compact_date() {
        assert_infer(
            &vals(&["20240115", "20240620"]),
            false,
            &[date_only(Compact)],
        );
    }

    #[test]
    fn infer_compact_datetime() {
        assert_infer(
            &vals(&["20240115T103000"]),
            false,
            &[dt(Compact, T, HmsCompact, None)],
        );
    }

    #[test]
    fn infer_unix_seconds() {
        assert_infer(
            &vals(&["1705312200", "1705398600"]),
            false,
            &[unix(Seconds)],
        );
    }

    #[test]
    fn infer_unix_ms() {
        assert_infer(
            &vals(&["1705312200000", "1705398600000"]),
            false,
            &[unix(Milliseconds)],
        );
    }

    #[test]
    fn infer_unix_us() {
        assert_infer(&vals(&["1705312200000000"]), false, &[unix(Microseconds)]);
    }

    #[test]
    fn infer_unix_ns() {
        assert_infer(&vals(&["1705312200000000000"]), false, &[unix(Nanoseconds)]);
    }

    // ── CSP resolution: ambiguity narrows over multiple values ─────────────

    #[test]
    fn slash_eu_resolved_midway() {
        assert_infer(
            &vals(&["01/02/2024", "15/03/2024"]),
            false,
            &[date_only(SlashEU)],
        );
    }

    #[test]
    fn slash_us_resolved_midway() {
        assert_infer(
            &vals(&["01/02/2024", "03/20/2024"]),
            false,
            &[date_only(SlashUS)],
        );
    }

    #[test]
    fn slash_eu_datetime_resolved_midway() {
        assert_infer(
            &vals(&["01/02/2024 10:00:00", "15/03/2024 11:00:00"]),
            false,
            &[dt(SlashEU, Space, Hms, None)],
        );
    }

    // ── Null and whitespace handling ────────────────────────────────────────

    #[test]
    fn nulls_skipped() {
        let input: Vec<Option<&str>> = vec![
            None,
            Some("2024-01-15T10:30:00"),
            None,
            Some("2024-06-20T08:00:00"),
            None,
        ];
        assert_infer(&input, false, &[dt(Iso, T, Hms, None)]);
    }

    #[test]
    fn whitespace_only_skipped() {
        let input: Vec<Option<&str>> = vec![Some("   "), Some("2024-01-15T10:30:00"), Some("\t\n")];
        assert_infer(&input, false, &[dt(Iso, T, Hms, None)]);
    }

    #[test]
    fn leading_trailing_whitespace_trimmed() {
        let input: Vec<Option<&str>> = vec![Some("  2024-01-15T10:30:00  ")];
        assert_infer(&input, false, &[dt(Iso, T, Hms, None)]);
    }

    // ── Empty column handling ───────────────────────────────────────────────

    #[test]
    fn all_nulls_returns_no_data() {
        let input: Vec<Option<&str>> = vec![None, None, None];
        assert!(matches!(infer(input.iter().copied(), false), InferResult::NoData));
    }

    #[test]
    fn empty_slice_returns_no_data() {
        let input: Vec<Option<&str>> = vec![];
        assert!(matches!(infer(input.iter().copied(), false), InferResult::NoData));
    }

    // ── No-match cases ──────────────────────────────────────────────────────

    #[test]
    fn incompatible_formats_no_match() {
        let input = vals(&["01/02/2024", "2024-01-15T10:30:00"]);
        assert!(matches!(infer(input.iter().copied(), false), InferResult::NoMatch));
    }

    #[test]
    fn garbage_no_match() {
        let input = vals(&["not a timestamp at all"]);
        assert!(matches!(infer(input.iter().copied(), false), InferResult::NoMatch));
    }

    // ── Multiple formats returned (ambiguous cases) ─────────────────────────

    #[test]
    fn ambiguous_slash_dates_returns_both() {
        let input = vals(&["01/02/2024", "03/04/2024", "05/06/2024"]);
        let result = infer(input.iter().copied(), false).into_formats();
        assert!(result.contains(&date_only(SlashUS)));
        assert!(result.contains(&date_only(SlashEU)));
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn ambiguous_slash_datetimes_returns_both() {
        let input = vals(&["01/02/2024 10:00:00", "03/04/2024 11:00:00"]);
        let result = infer(input.iter().copied(), false).into_formats();
        assert!(result.contains(&dt(SlashUS, Space, Hms, None)));
        assert!(result.contains(&dt(SlashEU, Space, Hms, None)));
        assert_eq!(result.len(), 2);
    }

    // ── Early-exit verification (non-exhaustive mode) ───────────────────────

    #[test]
    fn early_exit_iso8601_offset() {
        let input = vals(&["2024-01-15T10:30:00+05:30", "GARBAGE"]);
        assert_infer(&input, false, &[dt(Iso, T, Hms, Some(Offset))]);
    }

    #[test]
    fn early_exit_iso8601_frac_utc() {
        let input = vals(&["2024-01-15T10:30:00.123Z", "GARBAGE"]);
        assert_infer(&input, false, &[dt(Iso, T, HmsFrac, Some(Utc))]);
    }

    #[test]
    fn early_exit_space_offset() {
        let input = vals(&["2024-01-15 10:30:00+05:30", "GARBAGE"]);
        assert_infer(&input, false, &[dt(Iso, Space, Hms, Some(Offset))]);
    }

    #[test]
    fn early_exit_compact_datetime() {
        let input = vals(&["20240115T103000", "GARBAGE"]);
        assert_infer(&input, false, &[dt(Compact, T, HmsCompact, None)]);
    }

    #[test]
    fn early_exit_unix_ns() {
        let input = vals(&["1705312200000000000", "GARBAGE"]);
        assert_infer(&input, false, &[unix(Nanoseconds)]);
    }

    // ── Exhaustive mode tests ───────────────────────────────────────────────

    #[test]
    fn exhaustive_mode_processes_all_values() {
        let input = vals(&["2024-01-15T10:30:00+05:30", "GARBAGE"]);

        // Non-exhaustive exits after the first value resolves to one format.
        assert_infer(&input, false, &[dt(Iso, T, Hms, Some(Offset))]);

        // Exhaustive processes "GARBAGE" too, which eliminates all candidates.
        assert!(matches!(infer(input.iter().copied(), true), InferResult::NoMatch));
    }

    #[test]
    fn exhaustive_mode_returns_single_when_unique() {
        let input = vals(&["2024-01-15T10:30:00+05:30"]);

        assert_infer(&input, false, &[dt(Iso, T, Hms, Some(Offset))]);
        assert_infer(&input, true, &[dt(Iso, T, Hms, Some(Offset))]);
    }

    #[test]
    fn exhaustive_mode_continues_after_single_candidate() {
        let input = vals(&["2024-01-15T10:30:00+05:30", "2024-06-20T08:00:00+02:00"]);

        assert_infer(&input, false, &[dt(Iso, T, Hms, Some(Offset))]);
        assert_infer(&input, true, &[dt(Iso, T, Hms, Some(Offset))]);
    }

    #[test]
    fn exhaustive_empty_column_returns_no_data() {
        let input: Vec<Option<&str>> = vec![None, None];
        assert!(matches!(infer(input.iter().copied(), true), InferResult::NoData));
    }

    // ── 12-hour AM/PM inference ──────────────────────────────────────────────

    #[test]
    fn infer_slash_us_ampm() {
        assert_infer(
            &vals(&["01/15/2024 2:30:00 PM", "06/20/2024 8:00:00 AM"]),
            false,
            &[dt(SlashUS, Space, Hms12, None)],
        );
    }

    #[test]
    fn infer_slash_eu_ampm() {
        assert_infer(
            &vals(&["15/01/2024 2:30:00 PM", "20/06/2024 8:00:00 AM"]),
            false,
            &[dt(SlashEU, Space, Hms12, None)],
        );
    }

    #[test]
    fn infer_iso_ampm() {
        assert_infer(
            &vals(&["2024-01-15 2:30:00 PM", "2024-06-20 8:00:00 AM"]),
            false,
            &[dt(Iso, Space, Hms12, None)],
        );
    }

    #[test]
    fn infer_ampm_resolves_slash_ambiguity() {
        assert_infer(
            &vals(&["01/02/2024 3:00:00 AM", "03/15/2024 4:00:00 PM"]),
            false,
            &[dt(SlashUS, Space, Hms12, None)],
        );
    }

    #[test]
    fn early_exit_ampm() {
        let input = vals(&["2024-01-15 2:30:00 PM", "GARBAGE"]);
        assert_infer(&input, false, &[dt(Iso, Space, Hms12, None)]);
    }

    // ── 12-hour AM/PM compact (no space before AM/PM) ───────────────────────

    #[test]
    fn infer_slash_us_ampm_compact() {
        assert_infer(
            &vals(&["01/15/2024 2:30:00PM", "06/20/2024 8:00:00AM"]),
            false,
            &[dt(SlashUS, Space, Hms12Compact, None)],
        );
    }

    #[test]
    fn infer_iso_ampm_compact() {
        assert_infer(
            &vals(&["2024-01-15 2:30:00PM", "2024-06-20 8:00:00AM"]),
            false,
            &[dt(Iso, Space, Hms12Compact, None)],
        );
    }

    #[test]
    fn early_exit_ampm_compact() {
        let input = vals(&["2024-01-15 2:30:00PM", "GARBAGE"]);
        assert_infer(&input, false, &[dt(Iso, Space, Hms12Compact, None)]);
    }

    // ── Spaced timezone inference ───────────────────────────────────────────

    #[test]
    fn infer_spaced_tz_offset() {
        assert_infer(
            &vals(&["2024-01-15 10:30:00 +05:30", "2024-06-20 08:00:00 +02:00"]),
            false,
            &[dt_spaced_tz(Iso, Space, Hms, Offset)],
        );
    }

    #[test]
    fn infer_spaced_tz_offset_compact() {
        assert_infer(
            &vals(&["2024-01-15T10:30:00 +0530", "2024-06-20T08:00:00 +0200"]),
            false,
            &[dt_spaced_tz(Iso, T, Hms, OffsetCompact)],
        );
    }

    #[test]
    fn infer_rfc2822_spaced_tz() {
        assert_infer(
            &vals(&[
                "Mon, 15 Jan 2024 10:30:00 +0530",
                "Thu, 20 Jun 2024 08:00:00 -0800",
            ]),
            false,
            &[dt_spaced_tz(Rfc2822, Space, Hms, OffsetCompact)],
        );
    }
}
