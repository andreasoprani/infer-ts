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

// ─── Inference Engine ────────────────────────────────────────────────────────

/// Run hybrid format inference over a slice of (possibly null) string values.
///
/// `None` entries and whitespace-only strings are skipped (treated as nulls).
///
/// # Arguments
/// * `values` - Slice of optional string values to infer format from
/// * `exhaustive` - If `true`, process all values and return all compatible formats.
///   If `false`, return as soon as only one format remains (early exit).
///
/// # Returns
/// A `Vec<Format>` containing all formats compatible with the input values:
/// - If `exhaustive=false` and early-exit triggered: single-element Vec
/// - If `exhaustive=true` or no early exit: all surviving formats
/// - If no non-null values were seen: empty Vec (can't infer from no data)
/// - If no format matches all values: empty Vec
pub fn infer(values: &[Option<&str>], exhaustive: bool) -> Vec<Format> {
    let mut candidates: Option<HashSet<Format>> = None;

    for opt_value in values {
        let value = match opt_value {
            Some(v) => {
                let trimmed = v.trim();
                if trimmed.is_empty() {
                    continue;
                }
                trimmed
            }
            None => continue,
        };

        match &mut candidates {
            None => {
                // First non-null value: lazy domain construction via parsing
                candidates = Some(Format::parse(value).into_iter().collect());
            }
            Some(set) => {
                // Subsequent values: constraint propagation
                set.retain(|fmt| fmt.validates(value));
            }
        }

        if let Some(set) = &candidates {
            match set.len() {
                0 => return vec![],
                1 if !exhaustive => return vec![*set.iter().next().unwrap()],
                _ => {}
            }
        }
    }

    let Some(candidates) = candidates else {
        return vec![];
    };

    let mut result: Vec<Format> = candidates.into_iter().collect();
    result.sort();
    result
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
        let result = infer(values, exhaustive);
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
    fn all_nulls_returns_empty() {
        let input: Vec<Option<&str>> = vec![None, None, None];
        assert_eq!(infer(&input, false), vec![]);
    }

    #[test]
    fn empty_slice_returns_empty() {
        let input: Vec<Option<&str>> = vec![];
        assert_eq!(infer(&input, false), vec![]);
    }

    // ── No-match cases (return empty vec) ───────────────────────────────────

    #[test]
    fn incompatible_formats_no_match() {
        let input = vals(&["01/02/2024", "2024-01-15T10:30:00"]);
        assert_eq!(infer(&input, false), vec![]);
    }

    #[test]
    fn garbage_no_match() {
        let input = vals(&["not a timestamp at all"]);
        assert_eq!(infer(&input, false), vec![]);
    }

    // ── Multiple formats returned (ambiguous cases) ─────────────────────────

    #[test]
    fn ambiguous_slash_dates_returns_both() {
        let input = vals(&["01/02/2024", "03/04/2024", "05/06/2024"]);
        let result = infer(&input, false);
        assert!(result.contains(&date_only(SlashUS)));
        assert!(result.contains(&date_only(SlashEU)));
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn ambiguous_slash_datetimes_returns_both() {
        let input = vals(&["01/02/2024 10:00:00", "03/04/2024 11:00:00"]);
        let result = infer(&input, false);
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

        let non_exhaustive = infer(&input, false);
        assert_eq!(non_exhaustive, vec![dt(Iso, T, Hms, Some(Offset))]);

        let exhaustive = infer(&input, true);
        assert_eq!(exhaustive, vec![]);
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
    fn exhaustive_empty_column_returns_empty() {
        let input: Vec<Option<&str>> = vec![None, None];
        assert_eq!(infer(&input, true), vec![]);
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
