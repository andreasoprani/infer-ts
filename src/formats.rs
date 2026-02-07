//! Timestamp format definitions and per-value validation.
//!
//! Each [`Format`] variant represents an exact timestamp layout. Validation is
//! strict: a value only passes if its structure matches the format precisely, with
//! no extra or missing components.  This exactness is what lets the CSP elimination
//! in [`crate::inference`] work — each format's accepted set is as disjoint as
//! possible from the others.
//!
//! # Architecture
//!
//! Formats are organized compositionally:
//!
//! - **Standard formats** combine a date format, separator, time format, and optional
//!   timezone. All combinations are generated; the validator rejects impossible ones
//!   (e.g., slash dates with `T` separator). The CSP inference eliminates these on the
//!   first value anyway.
//!
//! - **Unix formats** are bare integers distinguished by digit count.

use std::sync::OnceLock;

use chrono::NaiveDate;

// ─── Component Enums ─────────────────────────────────────────────────────────

/// Date format within a standard timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DateFmt {
    /// `YYYY-MM-DD` (ISO 8601)
    Iso,
    /// `MM/DD/YYYY` (US convention)
    SlashUS,
    /// `DD/MM/YYYY` (EU convention)
    SlashEU,
    /// `YYYYMMDD` (compact, no separators)
    Compact,
}

/// Separator between date and time components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Separator {
    /// `T` or `t` (ISO 8601)
    T,
    /// Single space
    Space,
}

/// Time format within a standard timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeFmt {
    /// `HH:MM:SS`
    Hms,
    /// `HH:MM:SS.f` (1-9 fractional digits)
    HmsFrac,
    /// `HHMMSS` (compact, no colons) - only valid with Compact date
    HmsCompact,
}

/// Timezone suffix on a timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Timezone {
    /// `Z` or `z` (UTC)
    Utc,
    /// `±HH:MM` (with colon)
    Offset,
    /// `±HHMM` (compact, no colon)
    OffsetCompact,
}

/// Unix timestamp precision, distinguished by digit count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnixPrecision {
    /// 9–10 digits (seconds since epoch, ~1973–2286)
    Seconds,
    /// 11–13 digits (milliseconds)
    Milliseconds,
    /// 14–16 digits (microseconds)
    Microseconds,
    /// 17–19 digits (nanoseconds)
    Nanoseconds,
}

// ─── Component Fragments ────────────────────────────────────────────────────
//
// Each component knows how to contribute its fragment to a Polars format string.
// `StandardFormat::all_valid()` generates all possible combinations; the validator
// rejects impossible ones. Adding a new variant only requires implementing the
// fragment method and ensuring `validates()` handles it.

impl DateFmt {
    /// All date formats, in definition order.
    const fn all() -> &'static [DateFmt] {
        &[
            DateFmt::Iso,
            DateFmt::SlashUS,
            DateFmt::SlashEU,
            DateFmt::Compact,
        ]
    }

    /// Polars date fragment.
    fn polars_date(&self) -> &'static str {
        match self {
            DateFmt::Iso => "%Y-%m-%d",
            DateFmt::SlashUS => "%m/%d/%Y",
            DateFmt::SlashEU => "%d/%m/%Y",
            DateFmt::Compact => "%Y%m%d",
        }
    }
}

impl Separator {
    /// Polars separator fragment.
    fn polars_sep(&self) -> &'static str {
        match self {
            Separator::T => "T",
            Separator::Space => " ",
        }
    }
}

impl TimeFmt {
    /// Polars time fragment.
    fn polars_time(&self) -> &'static str {
        match self {
            TimeFmt::Hms => "%H:%M:%S",
            TimeFmt::HmsFrac => "%H:%M:%S%.f",
            TimeFmt::HmsCompact => "%H%M%S",
        }
    }
}

impl Timezone {
    /// Polars timezone fragment.
    fn polars_tz(&self) -> &'static str {
        match self {
            Timezone::Utc => "Z",
            Timezone::Offset => "%:z",
            Timezone::OffsetCompact => "%z",
        }
    }
}

impl UnixPrecision {
    /// All precisions, in definition order.
    const fn all() -> &'static [UnixPrecision] {
        &[
            UnixPrecision::Seconds,
            UnixPrecision::Milliseconds,
            UnixPrecision::Microseconds,
            UnixPrecision::Nanoseconds,
        ]
    }
}

// ─── Standard Format ─────────────────────────────────────────────────────────

/// A standard datetime format composed of date, separator, time, and optional timezone.
///
/// All combinations are constructible; the validator enforces which are actually parseable.
/// For example, `SlashUS + T separator` is impossible to parse but structurally valid.
/// The CSP inference eliminates impossible formats on the first value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StandardFormat {
    /// Date only (no time component)
    DateOnly { date: DateFmt },
    /// Date and time with optional timezone
    DateTime {
        date: DateFmt,
        sep: Separator,
        time: TimeFmt,
        tz: Option<Timezone>,
    },
}

// ─── Unix Format ─────────────────────────────────────────────────────────────

/// Unix epoch timestamp (bare integer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnixFormat {
    pub precision: UnixPrecision,
}

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

// ─── Named Format Constants ──────────────────────────────────────────────────
//
// These provide backwards-compatible names for the 21 standard formats.

#[allow(non_upper_case_globals, dead_code)]
impl Format {
    // ISO 8601 (T separator)
    pub const Iso8601DateTime: Format = Format::Standard(StandardFormat::DateTime {
        date: DateFmt::Iso,
        sep: Separator::T,
        time: TimeFmt::Hms,
        tz: None,
    });
    pub const Iso8601DateTimeUtc: Format = Format::Standard(StandardFormat::DateTime {
        date: DateFmt::Iso,
        sep: Separator::T,
        time: TimeFmt::Hms,
        tz: Some(Timezone::Utc),
    });
    pub const Iso8601DateTimeOffset: Format = Format::Standard(StandardFormat::DateTime {
        date: DateFmt::Iso,
        sep: Separator::T,
        time: TimeFmt::Hms,
        tz: Some(Timezone::Offset),
    });
    pub const Iso8601DateTimeOffsetCompact: Format = Format::Standard(StandardFormat::DateTime {
        date: DateFmt::Iso,
        sep: Separator::T,
        time: TimeFmt::Hms,
        tz: Some(Timezone::OffsetCompact),
    });
    pub const Iso8601DateTimeFrac: Format = Format::Standard(StandardFormat::DateTime {
        date: DateFmt::Iso,
        sep: Separator::T,
        time: TimeFmt::HmsFrac,
        tz: None,
    });
    pub const Iso8601DateTimeFracUtc: Format = Format::Standard(StandardFormat::DateTime {
        date: DateFmt::Iso,
        sep: Separator::T,
        time: TimeFmt::HmsFrac,
        tz: Some(Timezone::Utc),
    });
    pub const Iso8601DateTimeFracOffset: Format = Format::Standard(StandardFormat::DateTime {
        date: DateFmt::Iso,
        sep: Separator::T,
        time: TimeFmt::HmsFrac,
        tz: Some(Timezone::Offset),
    });
    pub const Iso8601DateTimeFracOffsetCompact: Format =
        Format::Standard(StandardFormat::DateTime {
            date: DateFmt::Iso,
            sep: Separator::T,
            time: TimeFmt::HmsFrac,
            tz: Some(Timezone::OffsetCompact),
        });

    // Space-separated
    pub const SpaceDateTime: Format = Format::Standard(StandardFormat::DateTime {
        date: DateFmt::Iso,
        sep: Separator::Space,
        time: TimeFmt::Hms,
        tz: None,
    });
    pub const SpaceDateTimeFrac: Format = Format::Standard(StandardFormat::DateTime {
        date: DateFmt::Iso,
        sep: Separator::Space,
        time: TimeFmt::HmsFrac,
        tz: None,
    });

    // Date-only
    pub const DateISO: Format = Format::Standard(StandardFormat::DateOnly { date: DateFmt::Iso });
    pub const DateSlashUS: Format = Format::Standard(StandardFormat::DateOnly {
        date: DateFmt::SlashUS,
    });
    pub const DateSlashEU: Format = Format::Standard(StandardFormat::DateOnly {
        date: DateFmt::SlashEU,
    });
    pub const DateCompact: Format = Format::Standard(StandardFormat::DateOnly {
        date: DateFmt::Compact,
    });

    // Slash datetime
    pub const DateTimeSlashUS: Format = Format::Standard(StandardFormat::DateTime {
        date: DateFmt::SlashUS,
        sep: Separator::Space,
        time: TimeFmt::Hms,
        tz: None,
    });
    pub const DateTimeSlashEU: Format = Format::Standard(StandardFormat::DateTime {
        date: DateFmt::SlashEU,
        sep: Separator::Space,
        time: TimeFmt::Hms,
        tz: None,
    });

    // Compact datetime
    pub const DateTimeCompact: Format = Format::Standard(StandardFormat::DateTime {
        date: DateFmt::Compact,
        sep: Separator::T,
        time: TimeFmt::HmsCompact,
        tz: None,
    });

    // Unix epoch
    pub const UnixSeconds: Format = Format::Unix(UnixFormat {
        precision: UnixPrecision::Seconds,
    });
    pub const UnixMilliseconds: Format = Format::Unix(UnixFormat {
        precision: UnixPrecision::Milliseconds,
    });
    pub const UnixMicroseconds: Format = Format::Unix(UnixFormat {
        precision: UnixPrecision::Microseconds,
    });
    pub const UnixNanoseconds: Format = Format::Unix(UnixFormat {
        precision: UnixPrecision::Nanoseconds,
    });
}

impl Format {
    /// Every supported format, generated from component constraints.
    ///
    /// Lazily initialised once via [`OnceLock`]; subsequent calls return the
    /// same slice.  Adding a new component variant and wiring its constraints
    /// in the `DateFmt` / `Separator` / … impls is sufficient to include every
    /// valid combination automatically.
    pub fn all() -> &'static [Format] {
        static FORMATS: OnceLock<Vec<Format>> = OnceLock::new();
        FORMATS.get_or_init(|| {
            let mut formats: Vec<Format> = StandardFormat::all_valid()
                .into_iter()
                .map(Format::Standard)
                .collect();
            for precision in UnixPrecision::all() {
                formats.push(Format::Unix(UnixFormat {
                    precision: *precision,
                }));
            }
            formats
        })
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
    /// This disjointness is essential for the CSP propagation step.
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

// ─── StandardFormat Implementation ───────────────────────────────────────────

impl StandardFormat {
    /// Generate all possible format combinations from component enums.
    /// Many combinations are impossible to parse (e.g., slash dates with `T` separator),
    /// but the validator rejects them naturally. The CSP inference eliminates impossible
    /// formats on the first value anyway, so the performance cost is negligible.
    fn all_valid() -> Vec<StandardFormat> {
        use Separator::*;
        use TimeFmt::*;
        use Timezone::*;

        let mut formats = Vec::new();

        // Date-only formats
        for date in DateFmt::all() {
            formats.push(StandardFormat::DateOnly { date: *date });
        }

        // All datetime combinations
        for date in DateFmt::all() {
            for sep in [T, Space] {
                for time in [Hms, HmsFrac, HmsCompact] {
                    for tz in [None, Some(Utc), Some(Offset), Some(OffsetCompact)] {
                        formats.push(StandardFormat::DateTime {
                            date: *date,
                            sep,
                            time,
                            tz,
                        });
                    }
                }
            }
        }

        formats
    }

    /// Polars-compatible format string, built compositionally from components.
    pub fn polars_format(&self) -> String {
        match self {
            StandardFormat::DateOnly { date } => date.polars_date().to_string(),
            StandardFormat::DateTime {
                date,
                sep,
                time,
                tz,
            } => {
                let mut s = date.polars_date().to_string();
                s.push_str(sep.polars_sep());
                s.push_str(time.polars_time());
                if let Some(tz) = tz {
                    s.push_str(tz.polars_tz());
                }
                s
            }
        }
    }

    /// Validate a value against this standard format.
    fn validates(&self, value: &str) -> bool {
        match self {
            StandardFormat::DateOnly { date } => match date {
                DateFmt::Iso => value.len() == 10 && parse_iso_date(value).is_some(),
                DateFmt::SlashUS => validate_slash_date(value, true),
                DateFmt::SlashEU => validate_slash_date(value, false),
                DateFmt::Compact => validate_compact_date(value),
            },
            StandardFormat::DateTime {
                date,
                sep,
                time,
                tz,
            } => {
                match date {
                    DateFmt::Iso => {
                        let Some(parsed) = parse_iso_like(value) else {
                            return false;
                        };
                        // Check separator
                        let sep_matches = match sep {
                            Separator::T => parsed.separator == ParsedSeparator::T,
                            Separator::Space => parsed.separator == ParsedSeparator::Space,
                        };
                        if !sep_matches {
                            return false;
                        }
                        // Space-separated formats don't support timezones
                        if matches!(sep, Separator::Space) && tz.is_some() {
                            return false;
                        }
                        // Check fractional
                        let frac_matches = match time {
                            TimeFmt::Hms => !parsed.has_frac,
                            TimeFmt::HmsFrac => parsed.has_frac,
                            TimeFmt::HmsCompact => return false, // Not valid for ISO date
                        };
                        if !frac_matches {
                            return false;
                        }
                        // Check timezone
                        match (tz, &parsed.suffix) {
                            (None, ParsedSuffix::None) => true,
                            (Some(Timezone::Utc), ParsedSuffix::UtcZ) => true,
                            (Some(Timezone::Offset), ParsedSuffix::OffsetColon) => true,
                            (Some(Timezone::OffsetCompact), ParsedSuffix::OffsetCompact) => true,
                            _ => false,
                        }
                    }
                    DateFmt::SlashUS => {
                        // Only valid with space separator, HMS time, no timezone
                        matches!((sep, time, tz), (Separator::Space, TimeFmt::Hms, None))
                            && validate_slash_datetime(value, true)
                    }
                    DateFmt::SlashEU => {
                        // Only valid with space separator, HMS time, no timezone
                        matches!((sep, time, tz), (Separator::Space, TimeFmt::Hms, None))
                            && validate_slash_datetime(value, false)
                    }
                    DateFmt::Compact => {
                        // Only valid with T separator, compact time, no timezone
                        matches!((sep, time, tz), (Separator::T, TimeFmt::HmsCompact, None))
                            && validate_compact_datetime(value)
                    }
                }
            }
        }
    }
}

// ─── UnixFormat Implementation ───────────────────────────────────────────────

impl UnixFormat {
    /// Polars-compatible format marker.
    pub fn polars_format(&self) -> &'static str {
        match self.precision {
            UnixPrecision::Seconds => "@unix_seconds",
            UnixPrecision::Milliseconds => "@unix_ms",
            UnixPrecision::Microseconds => "@unix_us",
            UnixPrecision::Nanoseconds => "@unix_ns",
        }
    }

    /// Validate a value as a Unix timestamp of this precision.
    fn validates(&self, value: &str) -> bool {
        let num_str = value.strip_prefix('-').unwrap_or(value);

        if num_str.is_empty() || !num_str.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
        // Reject leading zeros (except bare "0", which is < 9 digits anyway).
        if num_str.len() > 1 && num_str.as_bytes()[0] == b'0' {
            return false;
        }

        let digits = num_str.len();
        match self.precision {
            UnixPrecision::Seconds => (9..=10).contains(&digits),
            UnixPrecision::Milliseconds => (11..=13).contains(&digits),
            UnixPrecision::Microseconds => (14..=16).contains(&digits),
            UnixPrecision::Nanoseconds => (17..=19).contains(&digits),
        }
    }
}

// ─── Shared Parsing Primitives ───────────────────────────────────────────────

/// Structural description of a parsed ISO-like timestamp.
/// Internal type used during validation.
struct IsoStructure {
    separator: ParsedSeparator,
    has_frac: bool,
    suffix: ParsedSuffix,
}

/// Separator parsed from input (internal).
#[derive(PartialEq)]
enum ParsedSeparator {
    T,
    Space,
}

/// Timezone suffix parsed from input (internal).
#[derive(PartialEq)]
enum ParsedSuffix {
    None,
    UtcZ,
    /// Offset with colon: `+05:30` or `-08:00`
    OffsetColon,
    /// Compact offset without colon: `+0530` or `-0800`
    OffsetCompact,
}

/// Parse a `YYYY-MM-DD[T| ]HH:MM:SS[.f…][Z|±HH:MM]` string and return its
/// structural components.  Returns `None` on any syntax error.
fn parse_iso_like(s: &str) -> Option<IsoStructure> {
    // Minimum valid length: YYYY-MM-DDThh:mm:ss = 19 bytes
    if s.len() < 19 {
        return None;
    }

    // Date part (bytes 0..10)
    parse_iso_date(&s[..10])?;

    // Separator at byte 10
    let separator = match s.as_bytes()[10] {
        b'T' | b't' => ParsedSeparator::T,
        b' ' => ParsedSeparator::Space,
        _ => return None,
    };

    // Time part HH:MM:SS (bytes 11..19)
    parse_hms(&s[11..19])?;

    let mut pos: usize = 19;

    // Optional fractional seconds  .d{1,9}
    let has_frac = if pos < s.len() && s.as_bytes()[pos] == b'.' {
        let frac_digits = s[pos + 1..]
            .bytes()
            .take_while(|b| b.is_ascii_digit())
            .count();
        if frac_digits == 0 || frac_digits > 9 {
            return None;
        }
        pos += 1 + frac_digits;
        true
    } else {
        false
    };

    // Optional suffix: nothing / Z / ±offset
    let suffix = if pos == s.len() {
        ParsedSuffix::None
    } else {
        match s.as_bytes()[pos] {
            b'Z' | b'z' => {
                pos += 1;
                ParsedSuffix::UtcZ
            }
            b'+' | b'-' => {
                let (n, has_colon) = parse_tz_offset(&s[pos..])?;
                pos += n;
                if has_colon {
                    ParsedSuffix::OffsetColon
                } else {
                    ParsedSuffix::OffsetCompact
                }
            }
            _ => return None,
        }
    };

    // Must have consumed the entire string
    if pos != s.len() {
        return None;
    }

    Some(IsoStructure {
        separator,
        has_frac,
        suffix,
    })
}

/// Validate and return a `NaiveDate` from a `YYYY-MM-DD` string (exactly 10 bytes).
fn parse_iso_date(s: &str) -> Option<NaiveDate> {
    if s.len() != 10 || s.as_bytes()[4] != b'-' || s.as_bytes()[7] != b'-' {
        return None;
    }
    let year: i32 = s[0..4].parse().ok()?;
    let month: u32 = s[5..7].parse().ok()?;
    let day: u32 = s[8..10].parse().ok()?;
    NaiveDate::from_ymd_opt(year, month, day)
}

/// Validate an `HH:MM:SS` string (exactly 8 bytes).
fn parse_hms(s: &str) -> Option<()> {
    if s.len() != 8 || s.as_bytes()[2] != b':' || s.as_bytes()[5] != b':' {
        return None;
    }
    let h: u32 = s[0..2].parse().ok()?;
    let m: u32 = s[3..5].parse().ok()?;
    let sec: u32 = s[6..8].parse().ok()?;
    if h > 23 || m > 59 || sec > 59 {
        return None;
    }
    Some(())
}

/// Parse a timezone offset `±HH:MM` or `±HHMM`.
/// Returns `(bytes_consumed, has_colon)` where `has_colon` is true for `±HH:MM` format.
fn parse_tz_offset(s: &str) -> Option<(usize, bool)> {
    if s.len() < 5 || (s.as_bytes()[0] != b'+' && s.as_bytes()[0] != b'-') {
        return None;
    }
    let h: u32 = s[1..3].parse().ok()?;
    if h > 23 {
        return None;
    }

    if s.len() >= 6 && s.as_bytes()[3] == b':' {
        // ±HH:MM (with colon)
        let m: u32 = s[4..6].parse().ok()?;
        if m > 59 {
            return None;
        }
        Some((6, true))
    } else {
        // ±HHMM (compact, no colon)
        let m: u32 = s[3..5].parse().ok()?;
        if m > 59 {
            return None;
        }
        Some((5, false))
    }
}

// ─── Slash Date Validation ───────────────────────────────────────────────────

/// Parse slash date parts (AA/BB/CCCC) and return (a, b, year) if valid structure.
fn parse_slash_date_parts(s: &str) -> Option<(u32, u32, i32)> {
    if s.len() != 10 || s.as_bytes()[2] != b'/' || s.as_bytes()[5] != b'/' {
        return None;
    }
    let a: u32 = s[0..2].parse().ok()?;
    let b: u32 = s[3..5].parse().ok()?;
    let year: i32 = s[6..10].parse().ok()?;
    Some((a, b, year))
}

/// Validate a slash date (no time). `is_us` determines mm/dd/yyyy vs dd/mm/yyyy.
fn validate_slash_date(s: &str, is_us: bool) -> bool {
    let Some((a, b, year)) = parse_slash_date_parts(s) else {
        return false;
    };
    let (month, day) = if is_us { (a, b) } else { (b, a) };
    NaiveDate::from_ymd_opt(year, month, day).is_some()
}

/// Validate a slash datetime. `is_us` determines mm/dd/yyyy vs dd/mm/yyyy.
fn validate_slash_datetime(s: &str, is_us: bool) -> bool {
    let Some(space_pos) = s.find(' ') else {
        return false;
    };
    let date_part = &s[..space_pos];
    let time_part = &s[space_pos + 1..];

    let Some((a, b, year)) = parse_slash_date_parts(date_part) else {
        return false;
    };
    let (month, day) = if is_us { (a, b) } else { (b, a) };

    if NaiveDate::from_ymd_opt(year, month, day).is_none() {
        return false;
    }

    time_part.len() == 8 && parse_hms(time_part).is_some()
}

// ─── Compact Date / Datetime Validation ──────────────────────────────────────

fn validate_compact_date(s: &str) -> bool {
    if s.len() != 8 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    // Safety: we verified exactly 8 ASCII digits above.
    let year: i32 = s[0..4].parse().unwrap();
    let month: u32 = s[4..6].parse().unwrap();
    let day: u32 = s[6..8].parse().unwrap();
    NaiveDate::from_ymd_opt(year, month, day).is_some()
}

fn validate_compact_datetime(s: &str) -> bool {
    // YYYYMMDDThhmmss  →  exactly 15 bytes
    if s.len() != 15 || s.as_bytes()[8] != b'T' {
        return false;
    }
    if !validate_compact_date(&s[0..8]) {
        return false;
    }
    let time = &s[9..15];
    if !time.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let h: u32 = time[0..2].parse().unwrap();
    let m: u32 = time[2..4].parse().unwrap();
    let sec: u32 = time[4..6].parse().unwrap();
    h <= 23 && m <= 59 && sec <= 59
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helpers ─────────────────────────────────────────────────────────────

    /// Assert that *only* `expected` validates `value`; every other format rejects it.
    fn assert_only(value: &str, expected: Format) {
        for fmt in Format::all() {
            let ok = fmt.validates(value);
            if *fmt == expected {
                assert!(ok, "{:?} should accept {:?}", fmt, value);
            } else {
                assert!(
                    !ok,
                    "{:?} should reject {:?} (only {:?} expected)",
                    fmt, value, expected
                );
            }
        }
    }

    /// Assert that exactly the listed formats validate `value`.
    fn assert_set(value: &str, expected: &[Format]) {
        for fmt in Format::all() {
            let ok = fmt.validates(value);
            if expected.contains(fmt) {
                assert!(ok, "{:?} should accept {:?}", fmt, value);
            } else {
                assert!(!ok, "{:?} should reject {:?}", fmt, value);
            }
        }
    }

    /// Assert that *no* format validates `value`.
    fn assert_none(value: &str) {
        for fmt in Format::all() {
            assert!(!fmt.validates(value), "{:?} should reject {:?}", fmt, value);
        }
    }

    // ── ISO 8601 (T separator) ──────────────────────────────────────────────

    #[test]
    fn iso8601_plain() {
        assert_only("2024-01-15T10:30:00", Format::Iso8601DateTime);
    }

    #[test]
    fn iso8601_utc() {
        assert_only("2024-01-15T10:30:00Z", Format::Iso8601DateTimeUtc);
    }

    #[test]
    fn iso8601_offset_with_colon() {
        assert_only("2024-01-15T10:30:00+05:30", Format::Iso8601DateTimeOffset);
    }

    #[test]
    fn iso8601_offset_without_colon() {
        assert_only(
            "2024-01-15T10:30:00+0530",
            Format::Iso8601DateTimeOffsetCompact,
        );
    }

    #[test]
    fn iso8601_negative_offset() {
        assert_only("2024-01-15T10:30:00-08:00", Format::Iso8601DateTimeOffset);
    }

    #[test]
    fn iso8601_negative_offset_compact() {
        assert_only(
            "2024-01-15T10:30:00-0800",
            Format::Iso8601DateTimeOffsetCompact,
        );
    }

    #[test]
    fn iso8601_frac() {
        assert_only("2024-01-15T10:30:00.123456", Format::Iso8601DateTimeFrac);
    }

    #[test]
    fn iso8601_frac_utc() {
        assert_only(
            "2024-01-15T10:30:00.123456Z",
            Format::Iso8601DateTimeFracUtc,
        );
    }

    #[test]
    fn iso8601_frac_offset() {
        assert_only(
            "2024-01-15T10:30:00.123456+05:30",
            Format::Iso8601DateTimeFracOffset,
        );
    }

    #[test]
    fn iso8601_frac_offset_compact() {
        assert_only(
            "2024-01-15T10:30:00.123456+0530",
            Format::Iso8601DateTimeFracOffsetCompact,
        );
    }

    #[test]
    fn iso8601_lowercase_t() {
        assert_only("2024-01-15t10:30:00", Format::Iso8601DateTime);
    }

    #[test]
    fn iso8601_lowercase_z() {
        assert_only("2024-01-15T10:30:00z", Format::Iso8601DateTimeUtc);
    }

    #[test]
    fn iso8601_frac_1_digit() {
        assert_only("2024-01-15T10:30:00.1", Format::Iso8601DateTimeFrac);
    }

    #[test]
    fn iso8601_frac_9_digits() {
        assert_only("2024-01-15T10:30:00.123456789", Format::Iso8601DateTimeFrac);
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
        assert_only("2024-01-15 10:30:00", Format::SpaceDateTime);
    }

    #[test]
    fn space_frac() {
        assert_only("2024-01-15 10:30:00.999", Format::SpaceDateTimeFrac);
    }

    #[test]
    fn space_with_tz_rejected() {
        // Space + timezone is not a supported variant (yet)
        assert_none("2024-01-15 10:30:00+05:30");
    }

    // ── Date-only ISO ───────────────────────────────────────────────────────

    #[test]
    fn date_iso() {
        assert_only("2024-01-15", Format::DateISO);
    }

    #[test]
    fn date_iso_leap_feb29() {
        assert_only("2024-02-29", Format::DateISO);
    }

    #[test]
    fn date_iso_non_leap_feb29_rejected() {
        assert_none("2023-02-29");
    }

    #[test]
    fn date_iso_month_boundaries() {
        assert_only("2024-01-31", Format::DateISO);
        assert_none("2024-01-32");
        assert_none("2024-02-30");
        assert_only("2024-03-31", Format::DateISO);
        assert_none("2024-04-31");
    }

    // ── Slash dates: US vs EU disambiguation ────────────────────────────────

    #[test]
    fn slash_ambiguous_both_match() {
        // 01/02/2024 → US: Jan 2, EU: Feb 1 — both valid
        assert_set("01/02/2024", &[Format::DateSlashUS, Format::DateSlashEU]);
    }

    #[test]
    fn slash_us_only() {
        // day=15 > 12 kills EU (would need month=15)
        assert_only("01/15/2024", Format::DateSlashUS);
    }

    #[test]
    fn slash_eu_only() {
        // first=15 > 12 kills US (would need month=15)
        assert_only("15/01/2024", Format::DateSlashEU);
    }

    #[test]
    fn slash_us_datetime() {
        assert_only("01/15/2024 10:30:00", Format::DateTimeSlashUS);
    }

    #[test]
    fn slash_eu_datetime() {
        assert_only("15/01/2024 10:30:00", Format::DateTimeSlashEU);
    }

    #[test]
    fn slash_datetime_ambiguous() {
        assert_set(
            "01/02/2024 10:30:00",
            &[Format::DateTimeSlashUS, Format::DateTimeSlashEU],
        );
    }

    #[test]
    fn slash_bad_month_rejected() {
        // 13/01/2024 as US → month=13 invalid; as EU → day=13, month=01 → valid EU only
        assert_only("13/01/2024", Format::DateSlashEU);
    }

    // ── Compact ─────────────────────────────────────────────────────────────

    #[test]
    fn compact_date() {
        assert_only("20240115", Format::DateCompact);
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
        assert_only("20240115T103000", Format::DateTimeCompact);
    }

    #[test]
    fn compact_datetime_invalid_hour_rejected() {
        assert_none("20240115T253000");
    }

    // ── Unix epoch ──────────────────────────────────────────────────────────

    #[test]
    fn unix_seconds_10_digits() {
        assert_only("1705312200", Format::UnixSeconds);
    }

    #[test]
    fn unix_seconds_9_digits() {
        assert_only("170531220", Format::UnixSeconds);
    }

    #[test]
    fn unix_ms_13_digits() {
        assert_only("1705312200000", Format::UnixMilliseconds);
    }

    #[test]
    fn unix_ms_11_digits() {
        assert_only("17053122000", Format::UnixMilliseconds);
    }

    #[test]
    fn unix_us_16_digits() {
        assert_only("1705312200000000", Format::UnixMicroseconds);
    }

    #[test]
    fn unix_ns_19_digits() {
        assert_only("1705312200000000000", Format::UnixNanoseconds);
    }

    #[test]
    fn unix_negative_seconds() {
        assert_only("-170531220", Format::UnixSeconds);
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
}
