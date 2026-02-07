//! CSP-based timestamp format inference.
//!
//! The algorithm is a forward-checking constraint-propagation loop:
//!
//! 1. **Initialise** – seed the candidate set with every known [`Format`].
//! 2. **Propagate** – for each non-null value in the column:
//!    a. Remove every candidate whose [`Format::validates`] returns `false`.
//!    b. If the set is now empty → return empty list (no format fits all values).
//!    c. If exactly one candidate remains and `exhaustive=false` → return it
//!    immediately (**early exit**).
//! 3. **Finalise** – return all surviving candidates (sorted by name).
//!
//! Average-case complexity is *O(N × F)* (N = values, F = candidate formats), but
//! the early-exit means most real columns resolve within the first few rows.

use std::collections::HashSet;

use crate::formats::Format;

// ─── Inference Engine ────────────────────────────────────────────────────────

/// Run CSP format inference over a slice of (possibly null) string values.
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
/// - If `exhaustive=true` or no early exit: all surviving formats (sorted by name)
/// - If no non-null values were seen: all formats (empty column = all compatible)
/// - If no format matches all values: empty Vec
pub fn infer(values: &[Option<&str>], exhaustive: bool) -> Vec<Format> {
    let mut candidates: HashSet<Format> = Format::all().iter().copied().collect();

    for opt_value in values {
        // Extract and trim; skip nulls / blanks.
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

        // Constraint propagation: eliminate formats that reject this value.
        candidates.retain(|fmt| fmt.validates(value));

        match candidates.len() {
            0 => return vec![], // No format matches all values
            1 if !exhaustive => {
                // Early exit: return single-element Vec
                return vec![*candidates.iter().next().unwrap()];
            }
            _ => {} // keep narrowing
        }
    }

    // Return surviving candidates in enum definition order (as defined in Format::all())
    Format::all()
        .iter()
        .filter(|f| candidates.contains(f))
        .copied()
        .collect()
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
    fn std(date: DateFmt, sep: Separator, time: TimeFmt, tz: Option<Timezone>) -> Format {
        Format::Standard(StandardFormat::DateTime { date, sep, time, tz })
    }
    fn date_only(date: DateFmt) -> Format {
        Format::Standard(StandardFormat::DateOnly { date })
    }
    fn unix(precision: UnixPrecision) -> Format {
        Format::Unix(UnixFormat { precision })
    }

    use crate::formats::{DateFmt::{self, *}, Separator::{self, *}, TimeFmt::{self, *}, Timezone::{self, *}, UnixPrecision::{self, *}, StandardFormat, UnixFormat};

    // ── Happy-path: every format family resolves correctly ─────────────────

    #[test]
    fn infer_iso8601_plain() {
        assert_eq!(
            infer(
                &vals(&["2024-01-15T10:30:00", "2024-06-20T08:00:00"]),
                false
            ),
            vec![std(Iso, T, Hms, None)]
        );
    }

    #[test]
    fn infer_iso8601_utc() {
        assert_eq!(
            infer(&vals(&["2024-01-15T10:30:00Z"]), false),
            vec![std(Iso, T, Hms, Some(Utc))]
        );
    }

    #[test]
    fn infer_iso8601_offset() {
        assert_eq!(
            infer(&vals(&["2024-01-15T10:30:00+05:30"]), false),
            vec![std(Iso, T, Hms, Some(Offset))]
        );
    }

    #[test]
    fn infer_iso8601_frac() {
        assert_eq!(
            infer(&vals(&["2024-01-15T10:30:00.123"]), false),
            vec![std(Iso, T, HmsFrac, None)]
        );
    }

    #[test]
    fn infer_iso8601_frac_utc() {
        assert_eq!(
            infer(&vals(&["2024-01-15T10:30:00.123Z"]), false),
            vec![std(Iso, T, HmsFrac, Some(Utc))]
        );
    }

    #[test]
    fn infer_iso8601_frac_offset() {
        assert_eq!(
            infer(&vals(&["2024-01-15T10:30:00.123+05:30"]), false),
            vec![std(Iso, T, HmsFrac, Some(Offset))]
        );
    }

    #[test]
    fn infer_space_plain() {
        assert_eq!(
            infer(&vals(&["2024-01-15 10:30:00"]), false),
            vec![std(Iso, Space, Hms, None)]
        );
    }

    #[test]
    fn infer_space_frac() {
        assert_eq!(
            infer(&vals(&["2024-01-15 10:30:00.999999"]), false),
            vec![std(Iso, Space, HmsFrac, None)]
        );
    }

    #[test]
    fn infer_space_utc() {
        assert_eq!(
            infer(&vals(&["2024-01-15 10:30:00Z"]), false),
            vec![std(Iso, Space, Hms, Some(Utc))]
        );
    }

    #[test]
    fn infer_space_offset() {
        assert_eq!(
            infer(&vals(&["2024-01-15 10:30:00+05:30"]), false),
            vec![std(Iso, Space, Hms, Some(Offset))]
        );
    }

    #[test]
    fn infer_space_frac_utc() {
        assert_eq!(
            infer(&vals(&["2024-01-15 10:30:00.123Z"]), false),
            vec![std(Iso, Space, HmsFrac, Some(Utc))]
        );
    }

    #[test]
    fn infer_date_iso() {
        assert_eq!(
            infer(&vals(&["2024-01-15", "2024-06-20"]), false),
            vec![date_only(Iso)]
        );
    }

    #[test]
    fn infer_slash_us_unambiguous() {
        assert_eq!(
            infer(&vals(&["01/15/2024", "06/20/2024"]), false),
            vec![date_only(SlashUS)]
        );
    }

    #[test]
    fn infer_slash_eu_unambiguous() {
        assert_eq!(
            infer(&vals(&["15/01/2024", "20/06/2024"]), false),
            vec![date_only(SlashEU)]
        );
    }

    #[test]
    fn infer_slash_us_datetime() {
        assert_eq!(
            infer(&vals(&["01/15/2024 10:30:00"]), false),
            vec![std(SlashUS, Space, Hms, None)]
        );
    }

    #[test]
    fn infer_slash_eu_datetime() {
        assert_eq!(
            infer(&vals(&["15/01/2024 10:30:00"]), false),
            vec![std(SlashEU, Space, Hms, None)]
        );
    }

    #[test]
    fn infer_compact_date() {
        assert_eq!(
            infer(&vals(&["20240115", "20240620"]), false),
            vec![date_only(Compact)]
        );
    }

    #[test]
    fn infer_compact_datetime() {
        assert_eq!(
            infer(&vals(&["20240115T103000"]), false),
            vec![std(Compact, T, HmsCompact, None)]
        );
    }

    #[test]
    fn infer_unix_seconds() {
        assert_eq!(
            infer(&vals(&["1705312200", "1705398600"]), false),
            vec![unix(Seconds)]
        );
    }

    #[test]
    fn infer_unix_ms() {
        assert_eq!(
            infer(&vals(&["1705312200000", "1705398600000"]), false),
            vec![unix(Milliseconds)]
        );
    }

    #[test]
    fn infer_unix_us() {
        assert_eq!(
            infer(&vals(&["1705312200000000"]), false),
            vec![unix(Microseconds)]
        );
    }

    #[test]
    fn infer_unix_ns() {
        assert_eq!(
            infer(&vals(&["1705312200000000000"]), false),
            vec![unix(Nanoseconds)]
        );
    }

    // ── CSP resolution: ambiguity narrows over multiple values ─────────────

    #[test]
    fn slash_eu_resolved_midway() {
        // First value ambiguous (day ≤ 12 in both positions), second resolves to EU.
        assert_eq!(
            infer(&vals(&["01/02/2024", "15/03/2024"]), false),
            vec![date_only(SlashEU)]
        );
    }

    #[test]
    fn slash_us_resolved_midway() {
        // First value ambiguous, second resolves to US (day 20 in second slot).
        assert_eq!(
            infer(&vals(&["01/02/2024", "03/20/2024"]), false),
            vec![date_only(SlashUS)]
        );
    }

    #[test]
    fn slash_eu_datetime_resolved_midway() {
        assert_eq!(
            infer(
                &vals(&["01/02/2024 10:00:00", "15/03/2024 11:00:00"]),
                false
            ),
            vec![std(SlashEU, Space, Hms, None)]
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
        assert_eq!(infer(&input, false), vec![std(Iso, T, Hms, None)]);
    }

    #[test]
    fn whitespace_only_skipped() {
        let input: Vec<Option<&str>> = vec![Some("   "), Some("2024-01-15T10:30:00"), Some("\t\n")];
        assert_eq!(infer(&input, false), vec![std(Iso, T, Hms, None)]);
    }

    #[test]
    fn leading_trailing_whitespace_trimmed() {
        let input: Vec<Option<&str>> = vec![Some("  2024-01-15T10:30:00  ")];
        assert_eq!(infer(&input, false), vec![std(Iso, T, Hms, None)]);
    }

    // ── Empty column handling ───────────────────────────────────────────────

    #[test]
    fn all_nulls_returns_all_formats() {
        let input: Vec<Option<&str>> = vec![None, None, None];
        let result = infer(&input, false);
        assert_eq!(result.len(), Format::all().len());
    }

    #[test]
    fn empty_slice_returns_all_formats() {
        let input: Vec<Option<&str>> = vec![];
        let result = infer(&input, false);
        assert_eq!(result.len(), Format::all().len());
    }

    // ── No-match cases (return empty vec) ───────────────────────────────────

    #[test]
    fn incompatible_formats_no_match() {
        // Ambiguous slash date first (keeps US + EU alive, does NOT early-exit),
        // then an ISO datetime that neither slash format can parse → empty set.
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
        // Every day value ≤ 12 → US and EU both survive.
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
        assert!(result.contains(&std(SlashUS, Space, Hms, None)));
        assert!(result.contains(&std(SlashEU, Space, Hms, None)));
        assert_eq!(result.len(), 2);
    }

    // ── Early-exit verification (non-exhaustive mode) ───────────────────────
    // A format that resolves after one value should never read further values.
    // If early exit is broken the garbage value below would cause empty result.

    #[test]
    fn early_exit_iso8601_offset() {
        let input = vals(&["2024-01-15T10:30:00+05:30", "GARBAGE"]);
        assert_eq!(infer(&input, false), vec![std(Iso, T, Hms, Some(Offset))]);
    }

    #[test]
    fn early_exit_iso8601_frac_utc() {
        let input = vals(&["2024-01-15T10:30:00.123Z", "GARBAGE"]);
        assert_eq!(infer(&input, false), vec![std(Iso, T, HmsFrac, Some(Utc))]);
    }

    #[test]
    fn early_exit_space_offset() {
        let input = vals(&["2024-01-15 10:30:00+05:30", "GARBAGE"]);
        assert_eq!(infer(&input, false), vec![std(Iso, Space, Hms, Some(Offset))]);
    }

    #[test]
    fn early_exit_compact_datetime() {
        let input = vals(&["20240115T103000", "GARBAGE"]);
        assert_eq!(infer(&input, false), vec![std(Compact, T, HmsCompact, None)]);
    }

    #[test]
    fn early_exit_unix_ns() {
        let input = vals(&["1705312200000000000", "GARBAGE"]);
        assert_eq!(infer(&input, false), vec![unix(Nanoseconds)]);
    }

    // ── Exhaustive mode tests ───────────────────────────────────────────────

    #[test]
    fn exhaustive_mode_processes_all_values() {
        // With non-exhaustive, this returns early after first value
        let input = vals(&["2024-01-15T10:30:00+05:30", "GARBAGE"]);

        // Non-exhaustive: early exit, never sees GARBAGE
        let non_exhaustive = infer(&input, false);
        assert_eq!(non_exhaustive, vec![std(Iso, T, Hms, Some(Offset))]);

        // Exhaustive: processes all values including GARBAGE → no match
        let exhaustive = infer(&input, true);
        assert_eq!(exhaustive, vec![]);
    }

    #[test]
    fn exhaustive_mode_returns_single_when_unique() {
        // First value uniquely identifies ISO8601DateTimeOffset
        let input = vals(&["2024-01-15T10:30:00+05:30"]);

        // Both modes return the same result for unambiguous single-value input
        assert_eq!(infer(&input, false), vec![std(Iso, T, Hms, Some(Offset))]);
        assert_eq!(infer(&input, true), vec![std(Iso, T, Hms, Some(Offset))]);
    }

    #[test]
    fn exhaustive_mode_continues_after_single_candidate() {
        // After first value, only one format matches. Non-exhaustive exits.
        // Exhaustive continues and may find the remaining value also matches.
        let input = vals(&["2024-01-15T10:30:00+05:30", "2024-06-20T08:00:00+02:00"]);

        // Both should return the same single format
        assert_eq!(infer(&input, false), vec![std(Iso, T, Hms, Some(Offset))]);
        assert_eq!(infer(&input, true), vec![std(Iso, T, Hms, Some(Offset))]);
    }

    #[test]
    fn exhaustive_empty_column_returns_all_formats() {
        let input: Vec<Option<&str>> = vec![None, None];
        let result = infer(&input, true);
        assert_eq!(result.len(), Format::all().len());
    }

    // ── 12-hour AM/PM inference ──────────────────────────────────────────────

    #[test]
    fn infer_slash_us_ampm() {
        assert_eq!(
            infer(&vals(&["01/15/2024 2:30:00 PM", "06/20/2024 8:00:00 AM"]), false),
            vec![std(SlashUS, Space, Hms12, None)]
        );
    }

    #[test]
    fn infer_slash_eu_ampm() {
        assert_eq!(
            infer(&vals(&["15/01/2024 2:30:00 PM", "20/06/2024 8:00:00 AM"]), false),
            vec![std(SlashEU, Space, Hms12, None)]
        );
    }

    #[test]
    fn infer_iso_ampm() {
        assert_eq!(
            infer(&vals(&["2024-01-15 2:30:00 PM", "2024-06-20 8:00:00 AM"]), false),
            vec![std(Iso, Space, Hms12, None)]
        );
    }

    #[test]
    fn infer_ampm_resolves_slash_ambiguity() {
        // First value ambiguous (day ≤ 12), second resolves to US.
        assert_eq!(
            infer(&vals(&["01/02/2024 3:00:00 AM", "03/15/2024 4:00:00 PM"]), false),
            vec![std(SlashUS, Space, Hms12, None)]
        );
    }

    #[test]
    fn early_exit_ampm() {
        // ISO+Hms12 is unique on first value; GARBAGE never seen.
        let input = vals(&["2024-01-15 2:30:00 PM", "GARBAGE"]);
        assert_eq!(infer(&input, false), vec![std(Iso, Space, Hms12, None)]);
    }

    // ── 12-hour AM/PM compact (no space before AM/PM) ───────────────────────

    #[test]
    fn infer_slash_us_ampm_compact() {
        assert_eq!(
            infer(&vals(&["01/15/2024 2:30:00PM", "06/20/2024 8:00:00AM"]), false),
            vec![std(SlashUS, Space, Hms12Compact, None)]
        );
    }

    #[test]
    fn infer_iso_ampm_compact() {
        assert_eq!(
            infer(&vals(&["2024-01-15 2:30:00PM", "2024-06-20 8:00:00AM"]), false),
            vec![std(Iso, Space, Hms12Compact, None)]
        );
    }

    #[test]
    fn early_exit_ampm_compact() {
        let input = vals(&["2024-01-15 2:30:00PM", "GARBAGE"]);
        assert_eq!(infer(&input, false), vec![std(Iso, Space, Hms12Compact, None)]);
    }
}
