//! CSP-based timestamp format inference.
//!
//! The algorithm is a forward-checking constraint-propagation loop:
//!
//! 1. **Initialise** – seed the candidate set with every known [`Format`].
//! 2. **Propagate** – for each non-null value in the column:
//!    a. Remove every candidate whose [`Format::validates`] returns `false`.
//!    b. If the set is now empty → no format fits; return [`InferenceError::NoMatch`].
//!    c. If exactly one candidate remains → return it immediately (**early exit**).
//! 3. **Finalise** – after all values are processed, return the single survivor or
//!    an error if zero or more than one remain.
//!
//! Average-case complexity is *O(N × F)* (N = values, F = candidate formats), but
//! the early-exit means most real columns resolve within the first few rows.

use std::collections::HashSet;

use crate::formats::Format;

// ─── Error Type ──────────────────────────────────────────────────────────────

/// Errors produced by the inference engine.
#[derive(Debug)]
pub enum InferenceError {
    /// No single format is consistent with every non-null value.
    NoMatch,
    /// More than one format survived all constraints (column is ambiguous).
    Ambiguous(Vec<&'static str>),
    /// Every value in the slice was `None` or whitespace-only.
    EmptyColumn,
}

impl std::fmt::Display for InferenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoMatch => {
                write!(f, "no timestamp format matches all values in the column")
            }
            Self::Ambiguous(names) => {
                write!(
                    f,
                    "ambiguous: {} formats match all values: {:?}",
                    names.len(),
                    names
                )
            }
            Self::EmptyColumn => {
                write!(f, "cannot infer format — column contains only null/empty values")
            }
        }
    }
}

impl std::error::Error for InferenceError {}

impl From<InferenceError> for pyo3::PyErr {
    fn from(err: InferenceError) -> pyo3::PyErr {
        pyo3::exceptions::PyValueError::new_err(err.to_string())
    }
}

// ─── Inference Engine ────────────────────────────────────────────────────────

/// Run CSP format inference over a slice of (possibly null) string values.
///
/// `None` entries and whitespace-only strings are skipped (treated as nulls).
/// Returns the uniquely determined [`Format`] or an [`InferenceError`].
pub fn infer(values: &[Option<&str>]) -> Result<Format, InferenceError> {
    let mut candidates: HashSet<Format> = Format::all().iter().copied().collect();
    let mut seen_any = false;

    for opt_value in values {
        // Extract and trim; skip nulls / blanks.
        let value = match opt_value {
            Some(v) => {
                let trimmed = v.trim();
                if trimmed.is_empty() {
                    continue;
                }
                seen_any = true;
                trimmed
            }
            None => continue,
        };

        // Constraint propagation: eliminate formats that reject this value.
        candidates.retain(|fmt| fmt.validates(value));

        match candidates.len() {
            0 => return Err(InferenceError::NoMatch),
            1 => return Ok(*candidates.iter().next().unwrap()), // early exit
            _ => {} // keep narrowing
        }
    }

    if !seen_any {
        return Err(InferenceError::EmptyColumn);
    }

    match candidates.len() {
        0 => Err(InferenceError::NoMatch),
        1 => Ok(*candidates.iter().next().unwrap()),
        _ => {
            let mut names: Vec<&'static str> = candidates.iter().map(|f| f.name()).collect();
            names.sort_unstable();
            Err(InferenceError::Ambiguous(names))
        }
    }
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

    // ── Happy-path: every format family resolves correctly ─────────────────

    #[test]
    fn infer_iso8601_plain() {
        assert_eq!(
            infer(&vals(&["2024-01-15T10:30:00", "2024-06-20T08:00:00"])).unwrap(),
            Format::Iso8601DateTime
        );
    }

    #[test]
    fn infer_iso8601_utc() {
        assert_eq!(
            infer(&vals(&["2024-01-15T10:30:00Z"])).unwrap(),
            Format::Iso8601DateTimeUtc
        );
    }

    #[test]
    fn infer_iso8601_offset() {
        assert_eq!(
            infer(&vals(&["2024-01-15T10:30:00+05:30"])).unwrap(),
            Format::Iso8601DateTimeOffset
        );
    }

    #[test]
    fn infer_iso8601_frac() {
        assert_eq!(
            infer(&vals(&["2024-01-15T10:30:00.123"])).unwrap(),
            Format::Iso8601DateTimeFrac
        );
    }

    #[test]
    fn infer_iso8601_frac_utc() {
        assert_eq!(
            infer(&vals(&["2024-01-15T10:30:00.123Z"])).unwrap(),
            Format::Iso8601DateTimeFracUtc
        );
    }

    #[test]
    fn infer_iso8601_frac_offset() {
        assert_eq!(
            infer(&vals(&["2024-01-15T10:30:00.123+05:30"])).unwrap(),
            Format::Iso8601DateTimeFracOffset
        );
    }

    #[test]
    fn infer_space_plain() {
        assert_eq!(
            infer(&vals(&["2024-01-15 10:30:00"])).unwrap(),
            Format::SpaceDateTime
        );
    }

    #[test]
    fn infer_space_frac() {
        assert_eq!(
            infer(&vals(&["2024-01-15 10:30:00.999999"])).unwrap(),
            Format::SpaceDateTimeFrac
        );
    }

    #[test]
    fn infer_date_iso() {
        assert_eq!(
            infer(&vals(&["2024-01-15", "2024-06-20"])).unwrap(),
            Format::DateISO
        );
    }

    #[test]
    fn infer_slash_us_unambiguous() {
        assert_eq!(
            infer(&vals(&["01/15/2024", "06/20/2024"])).unwrap(),
            Format::DateSlashUS
        );
    }

    #[test]
    fn infer_slash_eu_unambiguous() {
        assert_eq!(
            infer(&vals(&["15/01/2024", "20/06/2024"])).unwrap(),
            Format::DateSlashEU
        );
    }

    #[test]
    fn infer_slash_us_datetime() {
        assert_eq!(
            infer(&vals(&["01/15/2024 10:30:00"])).unwrap(),
            Format::DateTimeSlashUS
        );
    }

    #[test]
    fn infer_slash_eu_datetime() {
        assert_eq!(
            infer(&vals(&["15/01/2024 10:30:00"])).unwrap(),
            Format::DateTimeSlashEU
        );
    }

    #[test]
    fn infer_compact_date() {
        assert_eq!(
            infer(&vals(&["20240115", "20240620"])).unwrap(),
            Format::DateCompact
        );
    }

    #[test]
    fn infer_compact_datetime() {
        assert_eq!(
            infer(&vals(&["20240115T103000"])).unwrap(),
            Format::DateTimeCompact
        );
    }

    #[test]
    fn infer_unix_seconds() {
        assert_eq!(
            infer(&vals(&["1705312200", "1705398600"])).unwrap(),
            Format::UnixSeconds
        );
    }

    #[test]
    fn infer_unix_ms() {
        assert_eq!(
            infer(&vals(&["1705312200000", "1705398600000"])).unwrap(),
            Format::UnixMilliseconds
        );
    }

    #[test]
    fn infer_unix_us() {
        assert_eq!(
            infer(&vals(&["1705312200000000"])).unwrap(),
            Format::UnixMicroseconds
        );
    }

    #[test]
    fn infer_unix_ns() {
        assert_eq!(
            infer(&vals(&["1705312200000000000"])).unwrap(),
            Format::UnixNanoseconds
        );
    }

    // ── CSP resolution: ambiguity narrows over multiple values ─────────────

    #[test]
    fn slash_eu_resolved_midway() {
        // First value ambiguous (day ≤ 12 in both positions), second resolves to EU.
        assert_eq!(
            infer(&vals(&["01/02/2024", "15/03/2024"])).unwrap(),
            Format::DateSlashEU
        );
    }

    #[test]
    fn slash_us_resolved_midway() {
        // First value ambiguous, second resolves to US (day 20 in second slot).
        assert_eq!(
            infer(&vals(&["01/02/2024", "03/20/2024"])).unwrap(),
            Format::DateSlashUS
        );
    }

    #[test]
    fn slash_eu_datetime_resolved_midway() {
        assert_eq!(
            infer(&vals(&["01/02/2024 10:00:00", "15/03/2024 11:00:00"])).unwrap(),
            Format::DateTimeSlashEU
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
        assert_eq!(infer(&input).unwrap(), Format::Iso8601DateTime);
    }

    #[test]
    fn whitespace_only_skipped() {
        let input: Vec<Option<&str>> = vec![
            Some("   "),
            Some("2024-01-15T10:30:00"),
            Some("\t\n"),
        ];
        assert_eq!(infer(&input).unwrap(), Format::Iso8601DateTime);
    }

    #[test]
    fn leading_trailing_whitespace_trimmed() {
        let input: Vec<Option<&str>> = vec![Some("  2024-01-15T10:30:00  ")];
        assert_eq!(infer(&input).unwrap(), Format::Iso8601DateTime);
    }

    // ── Error cases ─────────────────────────────────────────────────────────

    #[test]
    fn all_nulls_is_empty_column() {
        let input: Vec<Option<&str>> = vec![None, None, None];
        assert!(matches!(infer(&input), Err(InferenceError::EmptyColumn)));
    }

    #[test]
    fn empty_slice_is_empty_column() {
        let input: Vec<Option<&str>> = vec![];
        assert!(matches!(infer(&input), Err(InferenceError::EmptyColumn)));
    }

    #[test]
    fn incompatible_formats_no_match() {
        // Ambiguous slash date first (keeps US + EU alive, does NOT early-exit),
        // then an ISO datetime that neither slash format can parse → empty set.
        let input = vals(&["01/02/2024", "2024-01-15T10:30:00"]);
        assert!(matches!(infer(&input), Err(InferenceError::NoMatch)));
    }

    #[test]
    fn garbage_no_match() {
        let input = vals(&["not a timestamp at all"]);
        assert!(matches!(infer(&input), Err(InferenceError::NoMatch)));
    }

    #[test]
    fn ambiguous_slash_dates() {
        // Every day value ≤ 12 → US and EU both survive.
        let input = vals(&["01/02/2024", "03/04/2024", "05/06/2024"]);
        assert!(matches!(infer(&input), Err(InferenceError::Ambiguous(_))));
    }

    #[test]
    fn ambiguous_slash_datetimes() {
        let input = vals(&["01/02/2024 10:00:00", "03/04/2024 11:00:00"]);
        assert!(matches!(infer(&input), Err(InferenceError::Ambiguous(_))));
    }

    // ── Early-exit verification ─────────────────────────────────────────────
    // A format that resolves after one value should never read further values.
    // If early exit is broken the garbage value below would cause NoMatch.

    #[test]
    fn early_exit_iso8601_offset() {
        let input = vals(&["2024-01-15T10:30:00+05:30", "GARBAGE"]);
        assert_eq!(infer(&input).unwrap(), Format::Iso8601DateTimeOffset);
    }

    #[test]
    fn early_exit_iso8601_frac_utc() {
        let input = vals(&["2024-01-15T10:30:00.123Z", "GARBAGE"]);
        assert_eq!(infer(&input).unwrap(), Format::Iso8601DateTimeFracUtc);
    }

    #[test]
    fn early_exit_compact_datetime() {
        let input = vals(&["20240115T103000", "GARBAGE"]);
        assert_eq!(infer(&input).unwrap(), Format::DateTimeCompact);
    }

    #[test]
    fn early_exit_unix_ns() {
        let input = vals(&["1705312200000000000", "GARBAGE"]);
        assert_eq!(infer(&input).unwrap(), Format::UnixNanoseconds);
    }
}
