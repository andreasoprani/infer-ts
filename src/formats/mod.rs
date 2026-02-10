//! Timestamp format definitions, compositional parsing, and per-value validation.
//!
//! Each [`Format`] variant represents an exact timestamp layout. Parsing is
//! compositional: date → separator → time → timezone are parsed linearly,
//! accepting all structurally valid combinations.
//!
//! # Architecture
//!
//! - **Standard formats** combine a date format, separator, time format, and optional
//!   timezone. [`Format::parse`] discovers all matching formats for a value via
//!   compositional parsing. [`Format::validates`] checks a specific format against a value.
//!
//! - **Unix formats** are bare integers distinguished by digit count.

mod date;
mod standard;
mod unix;

pub use standard::StandardFormat;
pub use unix::{UnixFormat, UnixPrecision};

#[cfg(test)]
pub use date::{DateFmt, Separator, TimeFmt, Timezone};
#[cfg(test)]
pub use standard::TimeComponent;

// ─── Top-Level Format Enum ───────────────────────────────────────────────────

/// Every timestamp layout the inference engine can detect.
///
/// This is the top-level enum that encompasses both standard datetime formats
/// and Unix epoch timestamps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    Standard(StandardFormat),
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

        let mut matches: Vec<Format> = StandardFormat::parse(value)
            .into_iter()
            .map(Format::Standard)
            .collect();

        for precision in UnixPrecision::all() {
            let uf = UnixFormat {
                precision: *precision,
            };
            if uf.validates(value) {
                matches.push(Format::Unix(uf));
            }
        }

        matches
    }

    /// Polars-compatible format string for `Expr.str.to_datetime(format=...)`.
    ///
    /// Unix epoch formats return a special `@`-prefixed marker because Polars
    /// handles them via integer casting, not format-string parsing.
    pub fn polars_format(&self) -> String {
        match self {
            Format::Standard(std) => std.polars_format(),
            Format::Unix(unix) => unix.polars_format().to_string(),
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
            Format::Standard(std) => std.validates(value),
            Format::Unix(unix) => unix.validates(value),
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    use date::{DateFmt, Separator, TimeFmt, Timezone};

    // ── Helpers ─────────────────────────────────────────────────────────────

    // Format constructor helpers for concise test assertions
    fn std(date: DateFmt, sep: Separator, time: TimeFmt, tz: Option<Timezone>) -> Format {
        Format::Standard(StandardFormat::DateTime {
            date,
            time: TimeComponent {
                separator: sep,
                format: time,
                timezone: tz,
            },
        })
    }
    fn date_only(date: DateFmt) -> Format {
        Format::Standard(StandardFormat::DateOnly { date })
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
        assert_only("2024-01-15T10:30:00", std(Iso, T, Hms, None));
    }

    #[test]
    fn iso8601_utc() {
        assert_only("2024-01-15T10:30:00Z", std(Iso, T, Hms, Some(Utc)));
    }

    #[test]
    fn iso8601_offset_with_colon() {
        assert_only("2024-01-15T10:30:00+05:30", std(Iso, T, Hms, Some(Offset)));
    }

    #[test]
    fn iso8601_offset_without_colon() {
        assert_only(
            "2024-01-15T10:30:00+0530",
            std(Iso, T, Hms, Some(OffsetCompact)),
        );
    }

    #[test]
    fn iso8601_negative_offset() {
        assert_only("2024-01-15T10:30:00-08:00", std(Iso, T, Hms, Some(Offset)));
    }

    #[test]
    fn iso8601_negative_offset_compact() {
        assert_only(
            "2024-01-15T10:30:00-0800",
            std(Iso, T, Hms, Some(OffsetCompact)),
        );
    }

    #[test]
    fn iso8601_frac() {
        assert_only("2024-01-15T10:30:00.123456", std(Iso, T, HmsFrac, None));
    }

    #[test]
    fn iso8601_frac_utc() {
        assert_only(
            "2024-01-15T10:30:00.123456Z",
            std(Iso, T, HmsFrac, Some(Utc)),
        );
    }

    #[test]
    fn iso8601_frac_offset() {
        assert_only(
            "2024-01-15T10:30:00.123456+05:30",
            std(Iso, T, HmsFrac, Some(Offset)),
        );
    }

    #[test]
    fn iso8601_frac_offset_compact() {
        assert_only(
            "2024-01-15T10:30:00.123456+0530",
            std(Iso, T, HmsFrac, Some(OffsetCompact)),
        );
    }

    #[test]
    fn iso8601_lowercase_t() {
        assert_only("2024-01-15t10:30:00", std(Iso, T, Hms, None));
    }

    #[test]
    fn iso8601_lowercase_z() {
        assert_only("2024-01-15T10:30:00z", std(Iso, T, Hms, Some(Utc)));
    }

    #[test]
    fn iso8601_frac_1_digit() {
        assert_only("2024-01-15T10:30:00.1", std(Iso, T, HmsFrac, None));
    }

    #[test]
    fn iso8601_frac_9_digits() {
        assert_only("2024-01-15T10:30:00.123456789", std(Iso, T, HmsFrac, None));
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
        assert_only("2024-01-15 10:30:00", std(Iso, Space, Hms, None));
    }

    #[test]
    fn space_frac() {
        assert_only("2024-01-15 10:30:00.999", std(Iso, Space, HmsFrac, None));
    }

    #[test]
    fn space_utc() {
        assert_only("2024-01-15 10:30:00Z", std(Iso, Space, Hms, Some(Utc)));
    }

    #[test]
    fn space_offset() {
        assert_only(
            "2024-01-15 10:30:00+05:30",
            std(Iso, Space, Hms, Some(Offset)),
        );
    }

    #[test]
    fn space_offset_compact() {
        assert_only(
            "2024-01-15 10:30:00+0530",
            std(Iso, Space, Hms, Some(OffsetCompact)),
        );
    }

    #[test]
    fn space_frac_utc() {
        assert_only(
            "2024-01-15 10:30:00.123Z",
            std(Iso, Space, HmsFrac, Some(Utc)),
        );
    }

    #[test]
    fn space_frac_offset() {
        assert_only(
            "2024-01-15 10:30:00.123+05:30",
            std(Iso, Space, HmsFrac, Some(Offset)),
        );
    }

    #[test]
    fn space_frac_offset_compact() {
        assert_only(
            "2024-01-15 10:30:00.123+0530",
            std(Iso, Space, HmsFrac, Some(OffsetCompact)),
        );
    }

    #[test]
    fn space_negative_offset() {
        assert_only(
            "2024-01-15 10:30:00-08:00",
            std(Iso, Space, Hms, Some(Offset)),
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
        assert_only("01/15/2024 10:30:00", std(SlashUS, Space, Hms, None));
    }

    #[test]
    fn slash_eu_datetime() {
        assert_only("15/01/2024 10:30:00", std(SlashEU, Space, Hms, None));
    }

    #[test]
    fn slash_datetime_ambiguous() {
        assert_set(
            "01/02/2024 10:30:00",
            &[
                std(SlashUS, Space, Hms, None),
                std(SlashEU, Space, Hms, None),
            ],
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
        assert_only("20240115T103000", std(Compact, T, HmsCompact, None));
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
        assert_only("01/15/2024 2:30:00 PM", std(SlashUS, Space, Hms12, None));
    }

    #[test]
    fn slash_us_ampm_padded() {
        assert_only("01/15/2024 02:30:00 PM", std(SlashUS, Space, Hms12, None));
    }

    #[test]
    fn slash_eu_ampm() {
        assert_only("15/01/2024 2:30:00 PM", std(SlashEU, Space, Hms12, None));
    }

    #[test]
    fn slash_ampm_ambiguous() {
        assert_set(
            "01/02/2024 3:00:00 AM",
            &[
                std(SlashUS, Space, Hms12, None),
                std(SlashEU, Space, Hms12, None),
            ],
        );
    }

    #[test]
    fn iso_ampm() {
        assert_only("2024-01-15 2:30:00 PM", std(Iso, Space, Hms12, None));
    }

    #[test]
    fn iso_ampm_padded() {
        assert_only("2024-01-15 02:30:00 PM", std(Iso, Space, Hms12, None));
    }

    #[test]
    fn ampm_lowercase() {
        assert_only("01/15/2024 2:30:00 pm", std(SlashUS, Space, Hms12, None));
    }

    #[test]
    fn ampm_mixed_case() {
        assert_only("01/15/2024 2:30:00 Am", std(SlashUS, Space, Hms12, None));
    }

    #[test]
    fn ampm_hour_12() {
        assert_only("01/15/2024 12:00:00 PM", std(SlashUS, Space, Hms12, None));
    }

    #[test]
    fn ampm_hour_12_am() {
        assert_only("01/15/2024 12:00:00 AM", std(SlashUS, Space, Hms12, None));
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
        assert_only("2024-01-15T2:30:00 PM", std(Iso, T, Hms12, None));
    }

    #[test]
    fn ampm_tz_accepted() {
        // Compositional parser accepts 12-hour time with timezone
        assert_only(
            "2024-01-15 2:30:00 PM+05:00",
            std(Iso, Space, Hms12, Some(Offset)),
        );
        assert_only(
            "01/15/2024 2:30:00 PMZ",
            std(SlashUS, Space, Hms12, Some(Utc)),
        );
    }

    #[test]
    fn compact_date_ampm_accepted() {
        // Compositional parser accepts compact date with space + 12-hour time
        assert_only("20240115 2:30:00 PM", std(Compact, Space, Hms12, None));
    }

    // ── 12-hour AM/PM without space (compact) ────────────────────────────────

    #[test]
    fn slash_us_ampm_compact_unpadded() {
        assert_only(
            "01/15/2024 2:30:00PM",
            std(SlashUS, Space, Hms12Compact, None),
        );
    }

    #[test]
    fn slash_us_ampm_compact_padded() {
        assert_only(
            "01/15/2024 02:30:00PM",
            std(SlashUS, Space, Hms12Compact, None),
        );
    }

    #[test]
    fn slash_eu_ampm_compact() {
        assert_only(
            "15/01/2024 2:30:00PM",
            std(SlashEU, Space, Hms12Compact, None),
        );
    }

    #[test]
    fn iso_ampm_compact() {
        assert_only("2024-01-15 2:30:00PM", std(Iso, Space, Hms12Compact, None));
    }

    #[test]
    fn iso_ampm_compact_padded() {
        assert_only("2024-01-15 02:30:00PM", std(Iso, Space, Hms12Compact, None));
    }

    #[test]
    fn ampm_compact_lowercase() {
        assert_only(
            "01/15/2024 2:30:00pm",
            std(SlashUS, Space, Hms12Compact, None),
        );
    }

    #[test]
    fn ampm_compact_hour_12() {
        assert_only(
            "01/15/2024 12:00:00PM",
            std(SlashUS, Space, Hms12Compact, None),
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
        assert_only("01/15/24 10:30:00", std(SlashUSShort, Space, Hms, None));
    }

    #[test]
    fn slash_short_eu_datetime() {
        assert_only("15/01/24 10:30:00", std(SlashEUShort, Space, Hms, None));
    }

    #[test]
    fn slash_short_us_ampm_spaced() {
        assert_only("01/15/24 2:30:00 PM", std(SlashUSShort, Space, Hms12, None));
    }

    #[test]
    fn slash_short_eu_ampm_spaced() {
        assert_only("15/01/24 2:30:00 PM", std(SlashEUShort, Space, Hms12, None));
    }

    #[test]
    fn slash_short_us_ampm_compact() {
        assert_only(
            "01/15/24 2:30:00PM",
            std(SlashUSShort, Space, Hms12Compact, None),
        );
    }

    #[test]
    fn slash_short_eu_ampm_compact() {
        assert_only(
            "15/01/24 2:30:00PM",
            std(SlashEUShort, Space, Hms12Compact, None),
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
        assert_only("15.01.2024 10:30:00", std(DotEU, Space, Hms, None));
    }

    #[test]
    fn dot_eu_short_datetime() {
        assert_only("15.01.24 10:30:00", std(DotEUShort, Space, Hms, None));
    }

    #[test]
    fn dot_eu_ampm() {
        assert_only("15.01.2024 2:30:00 PM", std(DotEU, Space, Hms12, None));
    }

    #[test]
    fn dot_eu_short_ampm() {
        assert_only("15.01.24 2:30:00 PM", std(DotEUShort, Space, Hms12, None));
    }

    #[test]
    fn dot_eu_ampm_compact() {
        assert_only(
            "15.01.2024 2:30:00PM",
            std(DotEU, Space, Hms12Compact, None),
        );
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
        assert_only("Jan 15, 2024 10:30:00", std(MonthUS, Space, Hms, None));
    }

    #[test]
    fn month_eu_datetime() {
        assert_only("15 Jan 2024 10:30:00", std(MonthEU, Space, Hms, None));
    }

    #[test]
    fn month_us_short_year_datetime() {
        assert_only("Jan 15, 24 10:30:00", std(MonthUSShort, Space, Hms, None));
    }

    #[test]
    fn month_eu_short_year_datetime() {
        assert_only("15 Jan 24 10:30:00", std(MonthEUShort, Space, Hms, None));
    }

    #[test]
    fn month_us_ampm() {
        assert_only("Jan 15, 2024 2:30:00 PM", std(MonthUS, Space, Hms12, None));
    }

    #[test]
    fn month_eu_ampm() {
        assert_only("15 Jan 2024 2:30:00 PM", std(MonthEU, Space, Hms12, None));
    }

    #[test]
    fn month_us_ampm_compact() {
        assert_only(
            "Jan 15, 2024 2:30:00PM",
            std(MonthUS, Space, Hms12Compact, None),
        );
    }

    #[test]
    fn month_eu_ampm_compact() {
        assert_only(
            "15 Jan 2024 2:30:00PM",
            std(MonthEU, Space, Hms12Compact, None),
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
            std(MonthUSShort, Space, Hms12, None),
        );
    }

    #[test]
    fn month_eu_short_ampm() {
        assert_only(
            "15 Jan 24 2:30:00 PM",
            std(MonthEUShort, Space, Hms12, None),
        );
    }

    // ── RFC 2822 ──────────────────────────────────────────────────────────

    #[test]
    fn rfc2822_compact_offset() {
        assert_only(
            "Mon, 15 Jan 2024 10:30:00 +0530",
            std(Rfc2822, Space, Hms, Some(OffsetCompact)),
        );
    }

    #[test]
    fn rfc2822_colon_offset() {
        assert_only(
            "Mon, 15 Jan 2024 10:30:00 +05:30",
            std(Rfc2822, Space, Hms, Some(Offset)),
        );
    }

    #[test]
    fn rfc2822_negative_offset() {
        assert_only(
            "Mon, 15 Jan 2024 10:30:00 -0800",
            std(Rfc2822, Space, Hms, Some(OffsetCompact)),
        );
    }

    #[test]
    fn rfc2822_single_digit_day() {
        // Jan 1, 2024 is a Monday
        assert_only(
            "Mon, 1 Jan 2024 10:30:00 +0000",
            std(Rfc2822, Space, Hms, Some(OffsetCompact)),
        );
    }

    #[test]
    fn rfc2822_case_insensitive_dow_lower() {
        assert_only(
            "mon, 15 Jan 2024 10:30:00 +0530",
            std(Rfc2822, Space, Hms, Some(OffsetCompact)),
        );
    }

    #[test]
    fn rfc2822_case_insensitive_dow_upper() {
        assert_only(
            "MON, 15 JAN 2024 10:30:00 +0530",
            std(Rfc2822, Space, Hms, Some(OffsetCompact)),
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
            std(Rfc2822, Space, Hms, Some(OffsetCompact)),
        );
    }

    #[test]
    fn ampm_spaced_vs_compact_disjoint() {
        // " PM" (spaced) and "PM" (compact) never overlap
        assert_only("01/15/2024 2:30:00 PM", std(SlashUS, Space, Hms12, None));
        assert_only(
            "01/15/2024 2:30:00PM",
            std(SlashUS, Space, Hms12Compact, None),
        );
    }
}
