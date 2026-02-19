//! Timestamp format definitions, compositional parsing, and per-value validation.
//!
//! Each [`Format`] variant represents an exact timestamp layout. Parsing is
//! compositional: date → separator → time → timezone are parsed linearly,
//! accepting all structurally valid combinations.
//!
//! # Architecture
//!
//! - **Date formats** ([`DateFormat`]) represent date-only layouts.
//! - **DateTime formats** ([`DateTimeFormat`]) combine a date format, separator,
//!   time format, and optional timezone.
//! - [`Format::parse`] discovers all matching formats for a value via
//!   compositional parsing. [`Format::validates`] checks a specific format against a value.
//! - **Unix formats** are bare integers distinguished by digit count.

mod date;
mod datetime;
mod time;
mod unix;


pub use date::DateFormat;
pub use datetime::DateTimeFormat;
pub use unix::UnixFormat;

#[cfg(test)]
pub use date::DateFmt;
#[cfg(test)]
pub use datetime::TimeComponent;
#[cfg(test)]
pub use time::{Separator, TimeFmt, Timezone};
pub use unix::UnixPrecision;

// ─── Top-Level Format Enum ───────────────────────────────────────────────────

/// Every timestamp layout the inference engine can detect.
///
/// This is the top-level enum that encompasses date-only formats, datetime
/// formats, and Unix epoch timestamps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Format {
    Date(DateFormat),
    DateTime(DateTimeFormat),
    Unix(UnixFormat),
}

impl Format {
    /// Parse a value and return all matching formats.
    ///
    /// Uses compositional parsing for standard formats (date → separator → time → timezone)
    /// and digit-count matching for Unix formats.
    pub fn parse(value: &str) -> Vec<Format> {
        if value.is_empty() || !value.is_ascii() {
            return vec![];
        }

        let mut matches: Vec<Format> = DateFormat::parse(value)
            .into_iter()
            .map(Format::Date)
            .collect();

        matches.extend(
            DateTimeFormat::parse(value)
                .into_iter()
                .map(Format::DateTime),
        );

        matches.extend(UnixFormat::parse(value).into_iter().map(Format::Unix));

        matches
    }

    /// Polars-compatible format string for `Expr.str.to_datetime(format=...)`.
    ///
    /// Unix epoch formats return a special `@`-prefixed marker because Polars
    /// handles them via integer casting, not format-string parsing.
    pub fn polars_format(&self) -> String {
        match self {
            Format::Date(df) => df.polars_format(),
            Format::DateTime(dtf) => dtf.polars_format(),
            Format::Unix(unix) => unix.polars_format(),
        }
    }

    /// Parse `value` (already trimmed) to microseconds since the Unix epoch.
    ///
    /// Returns `None` if `value` is not a valid instance of this format.
    /// Timezone offsets in datetime strings are consumed but ignored — the
    /// wall-clock time is returned as-is (same semantics as Polars'
    /// `str.to_datetime(format=..., use_utc=false)`).
    pub fn parse_to_us(&self, value: &str) -> Option<i64> {
        match self {
            Format::Unix(uf) => {
                let n: i64 = value.parse().ok()?;
                match uf.precision {
                    UnixPrecision::Seconds => n.checked_mul(1_000_000),
                    UnixPrecision::Milliseconds => n.checked_mul(1_000),
                    UnixPrecision::Microseconds => Some(n),
                    UnixPrecision::Nanoseconds => Some(n / 1_000),
                }
            }
            Format::Date(df) => {
                let (date, remaining) = date::parse_date_value(value, df.date)?;
                if !remaining.is_empty() {
                    return None;
                }
                let midnight = chrono::NaiveTime::from_hms_opt(0, 0, 0)?;
                let dt = date.and_time(midnight);
                Some(dt.and_utc().timestamp_micros())
            }
            Format::DateTime(dtf) => {
                let (date, after_date) = date::parse_date_value(value, dtf.date)?;
                let after_sep = time::parse_separator(after_date, dtf.time.separator)?;
                let (t, after_time) = time::parse_time_value(after_sep, dtf.time.format)?;
                let remaining = match dtf.time.timezone {
                    None => after_time,
                    Some(tz) => {
                        let tz_input = if dtf.time.spaced_tz {
                            if after_time.is_empty() || after_time.as_bytes()[0] != b' ' {
                                return None;
                            }
                            &after_time[1..]
                        } else {
                            after_time
                        };
                        time::parse_timezone(tz_input, tz)?
                    }
                };
                if !remaining.is_empty() {
                    return None;
                }
                Some(date.and_time(t).and_utc().timestamp_micros())
            }
        }
    }

    /// Return `true` when `value` is a structurally valid instance of this format.
    ///
    /// Validation is **exact**: no extra or missing components are tolerated.
    ///
    /// **Note:** This method does *not* trim leading/trailing whitespace.  The
    /// inference engine ([`crate::inference::infer`]) trims values before calling
    /// this, so `"  2024-01-15  "` will fail here but succeed during inference.
    pub fn validates(&self, value: &str) -> bool {
        if value.is_empty() || !value.is_ascii() {
            return false;
        }

        match self {
            Format::Date(df) => df.validates(value),
            Format::DateTime(dtf) => dtf.validates(value),
            Format::Unix(unix) => unix.validates(value),
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    use date::DateFmt;
    use datetime::TimeComponent;
    use time::{Separator, TimeFmt, Timezone};

    // ── Helpers ─────────────────────────────────────────────────────────────

    // Format constructor helpers for concise test assertions
    fn dt(date: DateFmt, sep: Separator, time: TimeFmt, tz: Option<Timezone>) -> Format {
        Format::DateTime(DateTimeFormat {
            date,
            time: TimeComponent {
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
            time: TimeComponent {
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

    use super::{DateFmt::*, Separator::*, TimeFmt::*, Timezone::*, UnixPrecision::*};

    /// Assert that *only* `expected` validates `value`.
    fn assert_only(value: &str, expected: Format) {
        let parsed = Format::parse(value);
        assert_eq!(
            parsed,
            vec![expected],
            "expected only {:?} for {:?}, got {:?}",
            expected,
            value,
            parsed
        );
    }

    /// Assert that exactly the listed formats validate `value`.
    fn assert_set(value: &str, expected: &[Format]) {
        let parsed = Format::parse(value);
        assert_eq!(
            parsed.len(),
            expected.len(),
            "expected {:?} for {:?}, got {:?}",
            expected,
            value,
            parsed
        );
        for fmt in expected {
            assert!(
                parsed.contains(fmt),
                "{:?} should be in parse results for {:?}, got {:?}",
                fmt,
                value,
                parsed
            );
        }
    }

    /// Assert that *no* format validates `value`.
    fn assert_none(value: &str) {
        let parsed = Format::parse(value);
        assert!(
            parsed.is_empty(),
            "expected no formats for {:?}, got {:?}",
            value,
            parsed
        );
    }

    // ── ISO 8601 (T separator) ──────────────────────────────────────────────

    #[test]
    fn iso8601_plain() {
        assert_only("2024-01-15T10:30:00", dt(Iso, T, Hms, None));
    }

    #[test]
    fn iso8601_utc() {
        assert_only("2024-01-15T10:30:00Z", dt(Iso, T, Hms, Some(Utc)));
    }

    #[test]
    fn iso8601_offset_with_colon() {
        assert_only("2024-01-15T10:30:00+05:30", dt(Iso, T, Hms, Some(Offset)));
    }

    #[test]
    fn iso8601_offset_without_colon() {
        assert_only(
            "2024-01-15T10:30:00+0530",
            dt(Iso, T, Hms, Some(OffsetCompact)),
        );
    }

    #[test]
    fn iso8601_negative_offset() {
        assert_only("2024-01-15T10:30:00-08:00", dt(Iso, T, Hms, Some(Offset)));
    }

    #[test]
    fn iso8601_negative_offset_compact() {
        assert_only(
            "2024-01-15T10:30:00-0800",
            dt(Iso, T, Hms, Some(OffsetCompact)),
        );
    }

    #[test]
    fn iso8601_frac() {
        assert_only("2024-01-15T10:30:00.123456", dt(Iso, T, HmsFrac, None));
    }

    #[test]
    fn iso8601_frac_utc() {
        assert_only(
            "2024-01-15T10:30:00.123456Z",
            dt(Iso, T, HmsFrac, Some(Utc)),
        );
    }

    #[test]
    fn iso8601_frac_offset() {
        assert_only(
            "2024-01-15T10:30:00.123456+05:30",
            dt(Iso, T, HmsFrac, Some(Offset)),
        );
    }

    #[test]
    fn iso8601_frac_offset_compact() {
        assert_only(
            "2024-01-15T10:30:00.123456+0530",
            dt(Iso, T, HmsFrac, Some(OffsetCompact)),
        );
    }

    #[test]
    fn iso8601_lowercase_t() {
        assert_only("2024-01-15t10:30:00", dt(Iso, T, Hms, None));
    }

    #[test]
    fn iso8601_lowercase_z() {
        assert_only("2024-01-15T10:30:00z", dt(Iso, T, Hms, Some(Utc)));
    }

    #[test]
    fn iso8601_frac_1_digit() {
        assert_only("2024-01-15T10:30:00.1", dt(Iso, T, HmsFrac, None));
    }

    #[test]
    fn iso8601_frac_9_digits() {
        assert_only("2024-01-15T10:30:00.123456789", dt(Iso, T, HmsFrac, None));
    }

    #[test]
    fn iso8601_frac_dot_only_rejected() {
        assert_none("2024-01-15T10:30:00.");
    }

    #[test]
    fn iso8601_frac_10_digits_rejected() {
        assert_none("2024-01-15T10:30:00.1234567890");
    }

    // ── Space-separated ─────────────────────────────────────────────────────

    #[test]
    fn space_plain() {
        assert_only("2024-01-15 10:30:00", dt(Iso, Space, Hms, None));
    }

    #[test]
    fn space_frac() {
        assert_only("2024-01-15 10:30:00.999", dt(Iso, Space, HmsFrac, None));
    }

    #[test]
    fn space_utc() {
        assert_only("2024-01-15 10:30:00Z", dt(Iso, Space, Hms, Some(Utc)));
    }

    #[test]
    fn space_offset() {
        assert_only(
            "2024-01-15 10:30:00+05:30",
            dt(Iso, Space, Hms, Some(Offset)),
        );
    }

    #[test]
    fn space_offset_compact() {
        assert_only(
            "2024-01-15 10:30:00+0530",
            dt(Iso, Space, Hms, Some(OffsetCompact)),
        );
    }

    #[test]
    fn space_frac_utc() {
        assert_only(
            "2024-01-15 10:30:00.123Z",
            dt(Iso, Space, HmsFrac, Some(Utc)),
        );
    }

    #[test]
    fn space_frac_offset() {
        assert_only(
            "2024-01-15 10:30:00.123+05:30",
            dt(Iso, Space, HmsFrac, Some(Offset)),
        );
    }

    #[test]
    fn space_frac_offset_compact() {
        assert_only(
            "2024-01-15 10:30:00.123+0530",
            dt(Iso, Space, HmsFrac, Some(OffsetCompact)),
        );
    }

    #[test]
    fn space_negative_offset() {
        assert_only(
            "2024-01-15 10:30:00-08:00",
            dt(Iso, Space, Hms, Some(Offset)),
        );
    }

    // ── Date-only ISO ───────────────────────────────────────────────────────

    #[test]
    fn date_iso() {
        assert_only("2024-01-15", date_only(Iso));
    }

    #[test]
    fn date_iso_leap_feb29() {
        assert_only("2024-02-29", date_only(Iso));
    }

    #[test]
    fn date_iso_non_leap_feb29_rejected() {
        assert_none("2023-02-29");
    }

    #[test]
    fn date_iso_month_boundaries() {
        assert_only("2024-01-31", date_only(Iso));
        assert_none("2024-01-32");
        assert_none("2024-02-30");
        assert_only("2024-03-31", date_only(Iso));
        assert_none("2024-04-31");
    }

    // ── Slash dates: US vs EU disambiguation ────────────────────────────────

    #[test]
    fn slash_ambiguous_both_match() {
        // 01/02/2024 → US: Jan 2, EU: Feb 1 — both valid
        assert_set("01/02/2024", &[date_only(SlashUS), date_only(SlashEU)]);
    }

    #[test]
    fn slash_us_only() {
        // day=15 > 12 kills EU (would need month=15)
        assert_only("01/15/2024", date_only(SlashUS));
    }

    #[test]
    fn slash_eu_only() {
        // first=15 > 12 kills US (would need month=15)
        assert_only("15/01/2024", date_only(SlashEU));
    }

    #[test]
    fn slash_us_datetime() {
        assert_only("01/15/2024 10:30:00", dt(SlashUS, Space, Hms, None));
    }

    #[test]
    fn slash_eu_datetime() {
        assert_only("15/01/2024 10:30:00", dt(SlashEU, Space, Hms, None));
    }

    #[test]
    fn slash_datetime_ambiguous() {
        assert_set(
            "01/02/2024 10:30:00",
            &[dt(SlashUS, Space, Hms, None), dt(SlashEU, Space, Hms, None)],
        );
    }

    #[test]
    fn slash_bad_month_rejected() {
        // 13/01/2024 as US → month=13 invalid; as EU → day=13, month=01 → valid EU only
        assert_only("13/01/2024", date_only(SlashEU));
    }

    // ── Compact ─────────────────────────────────────────────────────────────

    #[test]
    fn compact_date() {
        assert_only("20240115", date_only(Compact));
    }

    #[test]
    fn compact_date_invalid_month_rejected() {
        assert_none("20241301");
    }

    #[test]
    fn compact_date_invalid_day_rejected() {
        assert_none("20240132");
    }

    #[test]
    fn compact_datetime() {
        assert_only("20240115T103000", dt(Compact, T, HmsCompact, None));
    }

    #[test]
    fn compact_datetime_invalid_hour_rejected() {
        assert_none("20240115T253000");
    }

    // ── Unix epoch ──────────────────────────────────────────────────────────

    #[test]
    fn unix_seconds_10_digits() {
        assert_only("1705312200", unix(Seconds));
    }

    #[test]
    fn unix_seconds_9_digits() {
        assert_only("170531220", unix(Seconds));
    }

    #[test]
    fn unix_ms_13_digits() {
        assert_only("1705312200000", unix(Milliseconds));
    }

    #[test]
    fn unix_ms_11_digits() {
        assert_only("17053122000", unix(Milliseconds));
    }

    #[test]
    fn unix_us_16_digits() {
        assert_only("1705312200000000", unix(Microseconds));
    }

    #[test]
    fn unix_ns_19_digits() {
        assert_only("1705312200000000000", unix(Nanoseconds));
    }

    #[test]
    fn unix_negative_seconds() {
        assert_only("-170531220", unix(Seconds));
    }

    #[test]
    fn unix_leading_zero_rejected() {
        assert_none("01705312200");
    }

    #[test]
    fn unix_8_digits_not_valid_date_rejected() {
        // 12345678 → year=1234, month=56 → invalid date, and < 9 digits for Unix
        assert_none("12345678");
    }

    // ── Global rejection cases ──────────────────────────────────────────────

    #[test]
    fn empty_string() {
        assert_none("");
    }

    #[test]
    fn non_ascii() {
        assert_none("2024-01-15T10:30:00é");
    }

    #[test]
    fn truncated_iso() {
        assert_none("2024-01-15T10:30");
    }

    #[test]
    fn invalid_hour() {
        assert_none("2024-01-15T25:00:00");
    }

    #[test]
    fn invalid_minute() {
        assert_none("2024-01-15T10:60:00");
    }

    #[test]
    fn invalid_second() {
        assert_none("2024-01-15T10:30:60");
    }

    #[test]
    fn garbage() {
        assert_none("not a timestamp");
        assert_none("hello world");
        assert_none("2024/01/15"); // slash with yyyy first — not a supported layout
    }

    // ── 12-hour AM/PM ─────────────────────────────────────────────────────────

    #[test]
    fn slash_us_ampm_unpadded() {
        assert_only("01/15/2024 2:30:00 PM", dt(SlashUS, Space, Hms12, None));
    }

    #[test]
    fn slash_us_ampm_padded() {
        assert_only("01/15/2024 02:30:00 PM", dt(SlashUS, Space, Hms12, None));
    }

    #[test]
    fn slash_eu_ampm() {
        assert_only("15/01/2024 2:30:00 PM", dt(SlashEU, Space, Hms12, None));
    }

    #[test]
    fn slash_ampm_ambiguous() {
        assert_set(
            "01/02/2024 3:00:00 AM",
            &[
                dt(SlashUS, Space, Hms12, None),
                dt(SlashEU, Space, Hms12, None),
            ],
        );
    }

    #[test]
    fn iso_ampm() {
        assert_only("2024-01-15 2:30:00 PM", dt(Iso, Space, Hms12, None));
    }

    #[test]
    fn iso_ampm_padded() {
        assert_only("2024-01-15 02:30:00 PM", dt(Iso, Space, Hms12, None));
    }

    #[test]
    fn ampm_lowercase() {
        assert_only("01/15/2024 2:30:00 pm", dt(SlashUS, Space, Hms12, None));
    }

    #[test]
    fn ampm_mixed_case() {
        assert_only("01/15/2024 2:30:00 Am", dt(SlashUS, Space, Hms12, None));
    }

    #[test]
    fn ampm_hour_12() {
        assert_only("01/15/2024 12:00:00 PM", dt(SlashUS, Space, Hms12, None));
    }

    #[test]
    fn ampm_hour_12_am() {
        assert_only("01/15/2024 12:00:00 AM", dt(SlashUS, Space, Hms12, None));
    }

    #[test]
    fn ampm_hour_0_rejected() {
        assert_none("01/15/2024 0:30:00 PM");
    }

    #[test]
    fn ampm_hour_13_rejected() {
        assert_none("01/15/2024 13:30:00 PM");
    }

    #[test]
    fn ampm_t_sep_accepted() {
        // Compositional parser accepts T separator with 12-hour time
        assert_only("2024-01-15T2:30:00 PM", dt(Iso, T, Hms12, None));
    }

    #[test]
    fn ampm_tz_accepted() {
        // Compositional parser accepts 12-hour time with timezone
        assert_only(
            "2024-01-15 2:30:00 PM+05:00",
            dt(Iso, Space, Hms12, Some(Offset)),
        );
        assert_only(
            "01/15/2024 2:30:00 PMZ",
            dt(SlashUS, Space, Hms12, Some(Utc)),
        );
    }

    #[test]
    fn compact_date_ampm_accepted() {
        // Compositional parser accepts compact date with space + 12-hour time
        assert_only("20240115 2:30:00 PM", dt(Compact, Space, Hms12, None));
    }

    // ── 12-hour AM/PM without space (compact) ────────────────────────────────

    #[test]
    fn slash_us_ampm_compact_unpadded() {
        assert_only(
            "01/15/2024 2:30:00PM",
            dt(SlashUS, Space, Hms12Compact, None),
        );
    }

    #[test]
    fn slash_us_ampm_compact_padded() {
        assert_only(
            "01/15/2024 02:30:00PM",
            dt(SlashUS, Space, Hms12Compact, None),
        );
    }

    #[test]
    fn slash_eu_ampm_compact() {
        assert_only(
            "15/01/2024 2:30:00PM",
            dt(SlashEU, Space, Hms12Compact, None),
        );
    }

    #[test]
    fn iso_ampm_compact() {
        assert_only("2024-01-15 2:30:00PM", dt(Iso, Space, Hms12Compact, None));
    }

    #[test]
    fn iso_ampm_compact_padded() {
        assert_only("2024-01-15 02:30:00PM", dt(Iso, Space, Hms12Compact, None));
    }

    #[test]
    fn ampm_compact_lowercase() {
        assert_only(
            "01/15/2024 2:30:00pm",
            dt(SlashUS, Space, Hms12Compact, None),
        );
    }

    #[test]
    fn ampm_compact_hour_12() {
        assert_only(
            "01/15/2024 12:00:00PM",
            dt(SlashUS, Space, Hms12Compact, None),
        );
    }

    #[test]
    fn ampm_compact_hour_0_rejected() {
        assert_none("01/15/2024 0:30:00PM");
    }

    #[test]
    fn ampm_compact_hour_13_rejected() {
        assert_none("01/15/2024 13:30:00PM");
    }

    // ── Short (2-digit year) slash dates ─────────────────────────────────

    #[test]
    fn slash_short_us_only() {
        // day=15 > 12 kills EU
        assert_only("01/15/24", date_only(SlashUSShort));
    }

    #[test]
    fn slash_short_eu_only() {
        // first=15 > 12 kills US
        assert_only("15/01/24", date_only(SlashEUShort));
    }

    #[test]
    fn slash_short_ambiguous_both_match() {
        // 01/02/24 → US: Jan 2, EU: Feb 1 — both valid
        assert_set(
            "01/02/24",
            &[date_only(SlashUSShort), date_only(SlashEUShort)],
        );
    }

    #[test]
    fn slash_short_us_datetime() {
        assert_only("01/15/24 10:30:00", dt(SlashUSShort, Space, Hms, None));
    }

    #[test]
    fn slash_short_eu_datetime() {
        assert_only("15/01/24 10:30:00", dt(SlashEUShort, Space, Hms, None));
    }

    #[test]
    fn slash_short_us_ampm_spaced() {
        assert_only("01/15/24 2:30:00 PM", dt(SlashUSShort, Space, Hms12, None));
    }

    #[test]
    fn slash_short_eu_ampm_spaced() {
        assert_only("15/01/24 2:30:00 PM", dt(SlashEUShort, Space, Hms12, None));
    }

    #[test]
    fn slash_short_us_ampm_compact() {
        assert_only(
            "01/15/24 2:30:00PM",
            dt(SlashUSShort, Space, Hms12Compact, None),
        );
    }

    #[test]
    fn slash_short_eu_ampm_compact() {
        assert_only(
            "15/01/24 2:30:00PM",
            dt(SlashEUShort, Space, Hms12Compact, None),
        );
    }

    #[test]
    fn slash_short_leap_year_valid() {
        // 2024 is a leap year (24 → 2024)
        assert_only("02/29/24", date_only(SlashUSShort));
    }

    #[test]
    fn slash_short_leap_year_invalid() {
        // 2023 is not a leap year (23 → 2023)
        assert_none("02/29/23");
    }

    #[test]
    fn slash_short_no_overlap_with_4digit() {
        // 8 chars vs 10 chars — should never overlap
        assert_only("01/15/24", date_only(SlashUSShort));
        assert_only("01/15/2024", date_only(SlashUS));
    }

    // ── Dot-separated European dates ──────────────────────────────────────

    #[test]
    fn dot_eu_date() {
        assert_only("15.01.2024", date_only(DotEU));
    }

    #[test]
    fn dot_eu_date_unambiguous() {
        // 01.02.2024 → day=01, month=02 — only DotEU (no DotUS exists)
        assert_only("01.02.2024", date_only(DotEU));
    }

    #[test]
    fn dot_eu_short_date() {
        assert_only("15.01.24", date_only(DotEUShort));
    }

    #[test]
    fn dot_eu_datetime() {
        assert_only("15.01.2024 10:30:00", dt(DotEU, Space, Hms, None));
    }

    #[test]
    fn dot_eu_short_datetime() {
        assert_only("15.01.24 10:30:00", dt(DotEUShort, Space, Hms, None));
    }

    #[test]
    fn dot_eu_ampm() {
        assert_only("15.01.2024 2:30:00 PM", dt(DotEU, Space, Hms12, None));
    }

    #[test]
    fn dot_eu_short_ampm() {
        assert_only("15.01.24 2:30:00 PM", dt(DotEUShort, Space, Hms12, None));
    }

    #[test]
    fn dot_eu_ampm_compact() {
        assert_only("15.01.2024 2:30:00PM", dt(DotEU, Space, Hms12Compact, None));
    }

    #[test]
    fn dot_eu_leap_year_valid() {
        assert_only("29.02.2024", date_only(DotEU));
    }

    #[test]
    fn dot_eu_leap_year_invalid() {
        assert_none("29.02.2023");
    }

    #[test]
    fn dot_eu_no_overlap_with_slash() {
        // Dot and slash formats never overlap
        assert_only("15.01.2024", date_only(DotEU));
        assert_only("15/01/2024", date_only(SlashEU));
    }

    // ── Month-name dates ──────────────────────────────────────────────────

    #[test]
    fn month_us_date() {
        assert_only("Jan 15, 2024", date_only(MonthUS));
    }

    #[test]
    fn month_eu_date() {
        assert_only("15 Jan 2024", date_only(MonthEU));
    }

    #[test]
    fn month_us_date_unpadded_day() {
        assert_only("Jan 5, 2024", date_only(MonthUS));
    }

    #[test]
    fn month_eu_date_unpadded_day() {
        assert_only("5 Jan 2024", date_only(MonthEU));
    }

    #[test]
    fn month_us_short_year() {
        assert_only("Jan 15, 24", date_only(MonthUSShort));
    }

    #[test]
    fn month_eu_short_year() {
        assert_only("15 Jan 24", date_only(MonthEUShort));
    }

    #[test]
    fn month_us_datetime() {
        assert_only("Jan 15, 2024 10:30:00", dt(MonthUS, Space, Hms, None));
    }

    #[test]
    fn month_eu_datetime() {
        assert_only("15 Jan 2024 10:30:00", dt(MonthEU, Space, Hms, None));
    }

    #[test]
    fn month_us_short_year_datetime() {
        assert_only("Jan 15, 24 10:30:00", dt(MonthUSShort, Space, Hms, None));
    }

    #[test]
    fn month_eu_short_year_datetime() {
        assert_only("15 Jan 24 10:30:00", dt(MonthEUShort, Space, Hms, None));
    }

    #[test]
    fn month_us_ampm() {
        assert_only("Jan 15, 2024 2:30:00 PM", dt(MonthUS, Space, Hms12, None));
    }

    #[test]
    fn month_eu_ampm() {
        assert_only("15 Jan 2024 2:30:00 PM", dt(MonthEU, Space, Hms12, None));
    }

    #[test]
    fn month_us_ampm_compact() {
        assert_only(
            "Jan 15, 2024 2:30:00PM",
            dt(MonthUS, Space, Hms12Compact, None),
        );
    }

    #[test]
    fn month_eu_ampm_compact() {
        assert_only(
            "15 Jan 2024 2:30:00PM",
            dt(MonthEU, Space, Hms12Compact, None),
        );
    }

    #[test]
    fn month_us_case_insensitive_lower() {
        assert_only("jan 15, 2024", date_only(MonthUS));
    }

    #[test]
    fn month_us_case_insensitive_upper() {
        assert_only("JAN 15, 2024", date_only(MonthUS));
    }

    #[test]
    fn month_eu_case_insensitive() {
        assert_only("15 jan 2024", date_only(MonthEU));
    }

    #[test]
    fn month_us_leap_year_valid() {
        assert_only("Feb 29, 2024", date_only(MonthUS));
    }

    #[test]
    fn month_us_leap_year_invalid() {
        assert_none("Feb 29, 2023");
    }

    #[test]
    fn month_eu_leap_year_valid() {
        assert_only("29 Feb 2024", date_only(MonthEU));
    }

    #[test]
    fn month_eu_leap_year_invalid() {
        assert_none("29 Feb 2023");
    }

    #[test]
    fn month_invalid_name_rejected() {
        assert_none("Xyz 15, 2024");
    }

    #[test]
    fn month_us_no_overlap_with_other_families() {
        // Month-name formats start with a letter, so no overlap with slash/dot/iso/compact
        assert_only("Jan 15, 2024", date_only(MonthUS));
        assert_only("01/15/2024", date_only(SlashUS));
    }

    #[test]
    fn month_eu_no_overlap_with_slash() {
        // EU month-name has month name, slash EU has digits
        assert_only("15 Jan 2024", date_only(MonthEU));
        assert_only("15/01/2024", date_only(SlashEU));
    }

    #[test]
    fn month_us_short_ampm() {
        assert_only(
            "Jan 15, 24 2:30:00 PM",
            dt(MonthUSShort, Space, Hms12, None),
        );
    }

    #[test]
    fn month_eu_short_ampm() {
        assert_only("15 Jan 24 2:30:00 PM", dt(MonthEUShort, Space, Hms12, None));
    }

    // ── RFC 2822 ──────────────────────────────────────────────────────────

    #[test]
    fn rfc2822_date_only() {
        assert_only("Mon, 15 Jan 2024", date_only(Rfc2822));
    }

    #[test]
    fn rfc2822_compact_offset() {
        assert_only(
            "Mon, 15 Jan 2024 10:30:00 +0530",
            dt_spaced_tz(Rfc2822, Space, Hms, OffsetCompact),
        );
    }

    #[test]
    fn rfc2822_colon_offset() {
        assert_only(
            "Mon, 15 Jan 2024 10:30:00 +05:30",
            dt_spaced_tz(Rfc2822, Space, Hms, Offset),
        );
    }

    #[test]
    fn rfc2822_negative_offset() {
        assert_only(
            "Mon, 15 Jan 2024 10:30:00 -0800",
            dt_spaced_tz(Rfc2822, Space, Hms, OffsetCompact),
        );
    }

    #[test]
    fn rfc2822_single_digit_day() {
        // Jan 1, 2024 is a Monday
        assert_only(
            "Mon, 1 Jan 2024 10:30:00 +0000",
            dt_spaced_tz(Rfc2822, Space, Hms, OffsetCompact),
        );
    }

    #[test]
    fn rfc2822_case_insensitive_dow_lower() {
        assert_only(
            "mon, 15 Jan 2024 10:30:00 +0530",
            dt_spaced_tz(Rfc2822, Space, Hms, OffsetCompact),
        );
    }

    #[test]
    fn rfc2822_case_insensitive_dow_upper() {
        assert_only(
            "MON, 15 JAN 2024 10:30:00 +0530",
            dt_spaced_tz(Rfc2822, Space, Hms, OffsetCompact),
        );
    }

    #[test]
    fn rfc2822_wrong_dow_rejected() {
        // Jan 15, 2024 was Monday, not Tuesday
        assert_none("Tue, 15 Jan 2024 10:30:00 +0530");
    }

    #[test]
    fn rfc2822_invalid_dow_rejected() {
        assert_none("Xyz, 15 Jan 2024 10:30:00 +0530");
    }

    #[test]
    fn rfc2822_no_overlap_with_month_eu() {
        // MonthEU: "15 Jan 2024" stays MonthEU
        assert_only("15 Jan 2024", date_only(MonthEU));
        // Rfc2822: "Mon, 15 Jan 2024 10:30:00 +0530" is Rfc2822
        assert_only(
            "Mon, 15 Jan 2024 10:30:00 +0530",
            dt_spaced_tz(Rfc2822, Space, Hms, OffsetCompact),
        );
    }

    #[test]
    fn ampm_spaced_vs_compact_disjoint() {
        // " PM" (spaced) and "PM" (compact) never overlap
        assert_only("01/15/2024 2:30:00 PM", dt(SlashUS, Space, Hms12, None));
        assert_only(
            "01/15/2024 2:30:00PM",
            dt(SlashUS, Space, Hms12Compact, None),
        );
    }

    // ── Failure edge cases ──────────────────────────────────────────────────

    #[test]
    fn trailing_garbage_rejected() {
        assert_none("2024-01-15T10:30:00 extra");
        assert_none("2024-01-15 extra");
        assert_none("01/15/2024 extra");
    }

    #[test]
    fn partial_timezone_rejected() {
        assert_none("2024-01-15T10:30:00+0");
        assert_none("2024-01-15T10:30:00+05");
        assert_none("2024-01-15T10:30:00+5:30");
    }

    #[test]
    fn invalid_timezone_values_rejected() {
        assert_none("2024-01-15T10:30:00+24:00");
        assert_none("2024-01-15T10:30:00+05:60");
    }

    #[test]
    fn month_zero_rejected() {
        assert_none("2024-00-15");
    }

    #[test]
    fn day_zero_rejected() {
        assert_none("2024-01-00");
    }

    #[test]
    fn feb_30_rejected() {
        assert_none("2024-02-30");
    }

    #[test]
    fn feb_31_rejected() {
        assert_none("2024-02-31");
    }

    #[test]
    fn slash_month_zero_rejected() {
        assert_none("00/15/2024");
    }

    #[test]
    fn slash_day_zero_rejected() {
        // US: month=01, day=00 → invalid; EU: day=01, month=00 → invalid
        assert_none("01/00/2024");
    }

    #[test]
    fn double_timezone_rejected() {
        assert_none("2024-01-15T10:30:00Z+05:30");
    }

    #[test]
    fn compact_datetime_invalid_minute_rejected() {
        assert_none("20240115T106000");
    }

    #[test]
    fn compact_datetime_invalid_second_rejected() {
        assert_none("20240115T103060");
    }

    #[test]
    fn dot_month_zero_rejected() {
        assert_none("15.00.2024");
    }

    #[test]
    fn dot_day_zero_rejected() {
        assert_none("00.01.2024");
    }

    #[test]
    fn iso_non_numeric_parts_rejected() {
        assert_none("20ab-01-15");
        assert_none("2024-ab-15");
        assert_none("2024-01-ab");
    }

    #[test]
    fn unix_too_few_digits_rejected() {
        // 8 digits is too few for unix (and might be a valid compact date)
        assert_none("12345678");
    }

    #[test]
    fn unix_20_digits_rejected() {
        assert_none("12345678901234567890");
    }
}
