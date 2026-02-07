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
    /// `H:MM:SS AM/PM` (12-hour, 1-2 digit hour, space before AM/PM)
    Hms12,
    /// `H:MM:SSAM/PM` (12-hour, 1-2 digit hour, no space before AM/PM)
    Hms12Compact,
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
            TimeFmt::Hms12 => "%I:%M:%S %p",
            TimeFmt::Hms12Compact => "%I:%M:%S%p",
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
                for time in [Hms, HmsFrac, HmsCompact, Hms12, Hms12Compact] {
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
                        // Hms12/Hms12Compact have variable-width hour; can't use parse_iso_like.
                        if matches!(time, TimeFmt::Hms12 | TimeFmt::Hms12Compact) {
                            if !matches!((sep, tz), (Separator::Space, None)) {
                                return false;
                            }
                            if value.len() < 11
                                || parse_iso_date(&value[..10]).is_none()
                                || value.as_bytes()[10] != b' '
                            {
                                return false;
                            }
                            let space = *time == TimeFmt::Hms12;
                            return parse_hms12(&value[11..], space)
                                == Some(value.len() - 11);
                        }

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
                        // Check fractional
                        let frac_matches = match time {
                            TimeFmt::Hms => !parsed.has_frac,
                            TimeFmt::HmsFrac => parsed.has_frac,
                            TimeFmt::HmsCompact => return false, // Not valid for ISO date
                            TimeFmt::Hms12 | TimeFmt::Hms12Compact => unreachable!(),
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
                    DateFmt::SlashUS => match (sep, time, tz) {
                        (Separator::Space, TimeFmt::Hms, None) => {
                            validate_slash_datetime(value, true)
                        }
                        (Separator::Space, TimeFmt::Hms12, None) => {
                            validate_slash_datetime_12h(value, true, true)
                        }
                        (Separator::Space, TimeFmt::Hms12Compact, None) => {
                            validate_slash_datetime_12h(value, true, false)
                        }
                        _ => false,
                    },
                    DateFmt::SlashEU => match (sep, time, tz) {
                        (Separator::Space, TimeFmt::Hms, None) => {
                            validate_slash_datetime(value, false)
                        }
                        (Separator::Space, TimeFmt::Hms12, None) => {
                            validate_slash_datetime_12h(value, false, true)
                        }
                        (Separator::Space, TimeFmt::Hms12Compact, None) => {
                            validate_slash_datetime_12h(value, false, false)
                        }
                        _ => false,
                    },
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

/// Parse a 12-hour time with AM/PM suffix (case-insensitive).
///
/// When `space_before_ampm` is `true`, expects `H:MM:SS AM` / `HH:MM:SS PM`.
/// When `false`, expects `H:MM:SSAM` / `HH:MM:SSPM` (no space).
///
/// Returns the number of bytes consumed on success, or `None` on failure.
fn parse_hms12(s: &str, space_before_ampm: bool) -> Option<usize> {
    let b = s.as_bytes();
    // Find the colon to determine hour width (1 or 2 digits)
    let colon1 = if b.len() > 1 && b[1] == b':' {
        1
    } else if b.len() > 2 && b[2] == b':' {
        2
    } else {
        return None;
    };

    let h: u32 = s[..colon1].parse().ok()?;
    if h < 1 || h > 12 {
        return None;
    }

    // MM:SS after first colon
    let rest = &s[colon1..];
    if rest.len() < 6 || rest.as_bytes()[0] != b':' || rest.as_bytes()[3] != b':' {
        return None;
    }
    let m: u32 = rest[1..3].parse().ok()?;
    let sec: u32 = rest[4..6].parse().ok()?;
    if m > 59 || sec > 59 {
        return None;
    }

    // AM/PM suffix: with or without leading space
    let suffix_start = colon1 + 6;
    if space_before_ampm {
        if s.len() < suffix_start + 3 || s.as_bytes()[suffix_start] != b' ' {
            return None;
        }
        let ampm = &s[suffix_start + 1..suffix_start + 3];
        if !ampm.eq_ignore_ascii_case("AM") && !ampm.eq_ignore_ascii_case("PM") {
            return None;
        }
        Some(suffix_start + 3)
    } else {
        if s.len() < suffix_start + 2 {
            return None;
        }
        let ampm = &s[suffix_start..suffix_start + 2];
        if !ampm.eq_ignore_ascii_case("AM") && !ampm.eq_ignore_ascii_case("PM") {
            return None;
        }
        Some(suffix_start + 2)
    }
}

/// Validate a slash datetime with 12-hour time. `is_us` determines mm/dd/yyyy vs dd/mm/yyyy.
/// `space_before_ampm` controls whether a space is expected before AM/PM.
fn validate_slash_datetime_12h(s: &str, is_us: bool, space_before_ampm: bool) -> bool {
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

    // parse_hms12 must consume the entire remaining string
    parse_hms12(time_part, space_before_ampm) == Some(time_part.len())
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

    use super::{DateFmt::{self, *}, Separator::{self, *}, TimeFmt::{self, *}, Timezone::{self, *}, UnixPrecision::{self, *}, StandardFormat, UnixFormat};

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
        assert_only("2024-01-15 10:30:00+05:30", std(Iso, Space, Hms, Some(Offset)));
    }

    #[test]
    fn space_offset_compact() {
        assert_only("2024-01-15 10:30:00+0530", std(Iso, Space, Hms, Some(OffsetCompact)));
    }

    #[test]
    fn space_frac_utc() {
        assert_only("2024-01-15 10:30:00.123Z", std(Iso, Space, HmsFrac, Some(Utc)));
    }

    #[test]
    fn space_frac_offset() {
        assert_only("2024-01-15 10:30:00.123+05:30", std(Iso, Space, HmsFrac, Some(Offset)));
    }

    #[test]
    fn space_frac_offset_compact() {
        assert_only("2024-01-15 10:30:00.123+0530", std(Iso, Space, HmsFrac, Some(OffsetCompact)));
    }

    #[test]
    fn space_negative_offset() {
        assert_only("2024-01-15 10:30:00-08:00", std(Iso, Space, Hms, Some(Offset)));
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
            &[std(SlashUS, Space, Hms, None), std(SlashEU, Space, Hms, None)],
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
    fn ampm_t_sep_rejected() {
        assert_none("2024-01-15T2:30:00 PM");
    }

    #[test]
    fn ampm_tz_rejected() {
        assert_none("2024-01-15 2:30:00 PM+05:00");
        assert_none("01/15/2024 2:30:00 PMZ");
    }

    #[test]
    fn compact_ampm_rejected() {
        assert_none("20240115 2:30:00 PM");
    }

    // ── 12-hour AM/PM without space (compact) ────────────────────────────────

    #[test]
    fn slash_us_ampm_compact_unpadded() {
        assert_only("01/15/2024 2:30:00PM", std(SlashUS, Space, Hms12Compact, None));
    }

    #[test]
    fn slash_us_ampm_compact_padded() {
        assert_only("01/15/2024 02:30:00PM", std(SlashUS, Space, Hms12Compact, None));
    }

    #[test]
    fn slash_eu_ampm_compact() {
        assert_only("15/01/2024 2:30:00PM", std(SlashEU, Space, Hms12Compact, None));
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
        assert_only("01/15/2024 2:30:00pm", std(SlashUS, Space, Hms12Compact, None));
    }

    #[test]
    fn ampm_compact_hour_12() {
        assert_only("01/15/2024 12:00:00PM", std(SlashUS, Space, Hms12Compact, None));
    }

    #[test]
    fn ampm_compact_hour_0_rejected() {
        assert_none("01/15/2024 0:30:00PM");
    }

    #[test]
    fn ampm_compact_hour_13_rejected() {
        assert_none("01/15/2024 13:30:00PM");
    }

    #[test]
    fn ampm_spaced_vs_compact_disjoint() {
        // " PM" (spaced) and "PM" (compact) never overlap
        assert_only("01/15/2024 2:30:00 PM", std(SlashUS, Space, Hms12, None));
        assert_only("01/15/2024 2:30:00PM", std(SlashUS, Space, Hms12Compact, None));
    }
}
