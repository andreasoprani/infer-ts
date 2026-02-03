//! Timestamp format definitions and per-value validation.
//!
//! Each [`Format`] variant represents an exact timestamp layout. Validation is
//! strict: a value only passes if its structure matches the format precisely, with
//! no extra or missing components.  This exactness is what lets the CSP elimination
//! in [`crate::inference`] work — each format's accepted set is as disjoint as
//! possible from the others.

use chrono::NaiveDate;

// ─── Format Enum ─────────────────────────────────────────────────────────────

/// Every timestamp layout the inference engine can detect.
///
/// Variants are grouped by family.  Within a group the order moves from
/// least-decorated to most-decorated so the match arms read naturally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    // ── ISO 8601 (T separator) ──────────────────────────────────────────────
    /// `2024-01-15T10:30:00`
    Iso8601DateTime,
    /// `2024-01-15T10:30:00Z`
    Iso8601DateTimeUtc,
    /// `2024-01-15T10:30:00+05:30`
    Iso8601DateTimeOffset,
    /// `2024-01-15T10:30:00.123456`
    Iso8601DateTimeFrac,
    /// `2024-01-15T10:30:00.123456Z`
    Iso8601DateTimeFracUtc,
    /// `2024-01-15T10:30:00.123456+05:30`
    Iso8601DateTimeFracOffset,

    // ── Space-separated ─────────────────────────────────────────────────────
    /// `2024-01-15 10:30:00`
    SpaceDateTime,
    /// `2024-01-15 10:30:00.123456`
    SpaceDateTimeFrac,

    // ── Date-only ───────────────────────────────────────────────────────────
    /// `2024-01-15`
    DateISO,

    // ── Slash-separated (US: mm/dd/yyyy  EU: dd/mm/yyyy) ───────────────────
    /// `01/15/2024`
    DateSlashUS,
    /// `15/01/2024`
    DateSlashEU,
    /// `01/15/2024 10:30:00`
    DateTimeSlashUS,
    /// `15/01/2024 10:30:00`
    DateTimeSlashEU,

    // ── Compact ─────────────────────────────────────────────────────────────
    /// `20240115`
    DateCompact,
    /// `20240115T103000`
    DateTimeCompact,

    // ── Unix epoch (bare integers) ──────────────────────────────────────────
    /// 9–10 digit integer  (seconds since epoch, ~1973–2286)
    UnixSeconds,
    /// 11–13 digit integer (milliseconds)
    UnixMilliseconds,
    /// 14–16 digit integer (microseconds)
    UnixMicroseconds,
    /// 17–19 digit integer (nanoseconds)
    UnixNanoseconds,
}

impl Format {
    /// Every supported format in a fixed, deterministic order.
    pub fn all() -> &'static [Format] {
        &[
            Format::Iso8601DateTime,
            Format::Iso8601DateTimeUtc,
            Format::Iso8601DateTimeOffset,
            Format::Iso8601DateTimeFrac,
            Format::Iso8601DateTimeFracUtc,
            Format::Iso8601DateTimeFracOffset,
            Format::SpaceDateTime,
            Format::SpaceDateTimeFrac,
            Format::DateISO,
            Format::DateSlashUS,
            Format::DateSlashEU,
            Format::DateTimeSlashUS,
            Format::DateTimeSlashEU,
            Format::DateCompact,
            Format::DateTimeCompact,
            Format::UnixSeconds,
            Format::UnixMilliseconds,
            Format::UnixMicroseconds,
            Format::UnixNanoseconds,
        ]
    }

    /// Human-readable label.
    pub fn name(&self) -> &'static str {
        match self {
            Format::Iso8601DateTime           => "ISO 8601 datetime",
            Format::Iso8601DateTimeUtc        => "ISO 8601 datetime (UTC Z)",
            Format::Iso8601DateTimeOffset     => "ISO 8601 datetime (offset)",
            Format::Iso8601DateTimeFrac       => "ISO 8601 datetime (fractional seconds)",
            Format::Iso8601DateTimeFracUtc    => "ISO 8601 datetime (fractional + UTC Z)",
            Format::Iso8601DateTimeFracOffset => "ISO 8601 datetime (fractional + offset)",
            Format::SpaceDateTime             => "Space-separated datetime",
            Format::SpaceDateTimeFrac         => "Space-separated datetime (fractional seconds)",
            Format::DateISO                   => "ISO 8601 date",
            Format::DateSlashUS               => "US slash date (mm/dd/yyyy)",
            Format::DateSlashEU               => "EU slash date (dd/mm/yyyy)",
            Format::DateTimeSlashUS           => "US slash datetime",
            Format::DateTimeSlashEU           => "EU slash datetime",
            Format::DateCompact               => "Compact date (yyyymmdd)",
            Format::DateTimeCompact           => "Compact datetime (yyyymmddThhmmss)",
            Format::UnixSeconds               => "Unix epoch seconds",
            Format::UnixMilliseconds          => "Unix epoch milliseconds",
            Format::UnixMicroseconds          => "Unix epoch microseconds",
            Format::UnixNanoseconds           => "Unix epoch nanoseconds",
        }
    }

    /// Polars-compatible format string for `Expr.str.to_datetime(format=...)`.
    ///
    /// Unix epoch formats return a special `@`-prefixed marker because Polars
    /// handles them via integer casting, not format-string parsing.
    pub fn polars_format(&self) -> &'static str {
        match self {
            Format::Iso8601DateTime           => "%Y-%m-%dT%H:%M:%S",
            Format::Iso8601DateTimeUtc        => "%Y-%m-%dT%H:%M:%SZ",
            Format::Iso8601DateTimeOffset     => "%Y-%m-%dT%H:%M:%S%:z",
            Format::Iso8601DateTimeFrac       => "%Y-%m-%dT%H:%M:%S%.f",
            Format::Iso8601DateTimeFracUtc    => "%Y-%m-%dT%H:%M:%S%.fZ",
            Format::Iso8601DateTimeFracOffset => "%Y-%m-%dT%H:%M:%S%.f%:z",
            Format::SpaceDateTime             => "%Y-%m-%d %H:%M:%S",
            Format::SpaceDateTimeFrac         => "%Y-%m-%d %H:%M:%S%.f",
            Format::DateISO                   => "%Y-%m-%d",
            Format::DateSlashUS               => "%m/%d/%Y",
            Format::DateSlashEU               => "%d/%m/%Y",
            Format::DateTimeSlashUS           => "%m/%d/%Y %H:%M:%S",
            Format::DateTimeSlashEU           => "%d/%m/%Y %H:%M:%S",
            Format::DateCompact               => "%Y%m%d",
            Format::DateTimeCompact           => "%Y%m%dT%H%M%S",
            Format::UnixSeconds               => "@unix_seconds",
            Format::UnixMilliseconds          => "@unix_ms",
            Format::UnixMicroseconds          => "@unix_us",
            Format::UnixNanoseconds           => "@unix_ns",
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
            // ── ISO 8601 group (T separator) ────────────────────────────────
            Format::Iso8601DateTime
            | Format::Iso8601DateTimeUtc
            | Format::Iso8601DateTimeOffset
            | Format::Iso8601DateTimeFrac
            | Format::Iso8601DateTimeFracUtc
            | Format::Iso8601DateTimeFracOffset => {
                let Some(ts) = parse_iso_like(value) else {
                    return false;
                };
                ts.separator == Separator::T
                    && match self {
                        Format::Iso8601DateTime           => !ts.has_frac && ts.suffix == Suffix::None,
                        Format::Iso8601DateTimeUtc        => !ts.has_frac && ts.suffix == Suffix::UtcZ,
                        Format::Iso8601DateTimeOffset     => !ts.has_frac && ts.suffix == Suffix::Offset,
                        Format::Iso8601DateTimeFrac       => ts.has_frac  && ts.suffix == Suffix::None,
                        Format::Iso8601DateTimeFracUtc    => ts.has_frac  && ts.suffix == Suffix::UtcZ,
                        Format::Iso8601DateTimeFracOffset => ts.has_frac  && ts.suffix == Suffix::Offset,
                        _ => unreachable!(),
                    }
            }

            // ── Space-separated group ───────────────────────────────────────
            Format::SpaceDateTime | Format::SpaceDateTimeFrac => {
                let Some(ts) = parse_iso_like(value) else {
                    return false;
                };
                ts.separator == Separator::Space
                    && ts.suffix == Suffix::None
                    && match self {
                        Format::SpaceDateTime     => !ts.has_frac,
                        Format::SpaceDateTimeFrac => ts.has_frac,
                        _ => unreachable!(),
                    }
            }

            // ── Date-only ───────────────────────────────────────────────────
            Format::DateISO => value.len() == 10 && parse_iso_date(value).is_some(),

            // ── Slash-separated ─────────────────────────────────────────────
            Format::DateSlashUS
            | Format::DateSlashEU
            | Format::DateTimeSlashUS
            | Format::DateTimeSlashEU => validate_slash(value, self),

            // ── Compact ─────────────────────────────────────────────────────
            Format::DateCompact     => validate_compact_date(value),
            Format::DateTimeCompact => validate_compact_datetime(value),

            // ── Unix epoch ──────────────────────────────────────────────────
            Format::UnixSeconds
            | Format::UnixMilliseconds
            | Format::UnixMicroseconds
            | Format::UnixNanoseconds => validate_unix(value, self),
        }
    }
}

// ─── Shared Parsing Primitives ───────────────────────────────────────────────

/// Structural description of a parsed ISO-like timestamp.
struct IsoStructure {
    separator: Separator,
    has_frac:  bool,
    suffix:    Suffix,
}

#[derive(PartialEq)]
enum Separator {
    T,
    Space,
}

#[derive(PartialEq)]
enum Suffix {
    None,
    UtcZ,
    Offset,
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
        b'T' | b't' => Separator::T,
        b' '        => Separator::Space,
        _           => return None,
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
        Suffix::None
    } else {
        match s.as_bytes()[pos] {
            b'Z' | b'z' => {
                pos += 1;
                Suffix::UtcZ
            }
            b'+' | b'-' => {
                let n = parse_tz_offset(&s[pos..])?;
                pos += n;
                Suffix::Offset
            }
            _ => return None,
        }
    };

    // Must have consumed the entire string
    if pos != s.len() {
        return None;
    }

    Some(IsoStructure { separator, has_frac, suffix })
}

/// Validate and return a `NaiveDate` from a `YYYY-MM-DD` string (exactly 10 bytes).
fn parse_iso_date(s: &str) -> Option<NaiveDate> {
    if s.len() != 10 || s.as_bytes()[4] != b'-' || s.as_bytes()[7] != b'-' {
        return None;
    }
    let year:  i32  = s[0..4].parse().ok()?;
    let month: u32  = s[5..7].parse().ok()?;
    let day:   u32  = s[8..10].parse().ok()?;
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

/// Parse a timezone offset `±HH:MM` or `±HHMM`.  Returns the byte length consumed.
fn parse_tz_offset(s: &str) -> Option<usize> {
    if s.len() < 5 || (s.as_bytes()[0] != b'+' && s.as_bytes()[0] != b'-') {
        return None;
    }
    let h: u32 = s[1..3].parse().ok()?;
    if h > 23 {
        return None;
    }

    if s.len() >= 6 && s.as_bytes()[3] == b':' {
        // ±HH:MM
        let m: u32 = s[4..6].parse().ok()?;
        if m > 59 { return None; }
        Some(6)
    } else {
        // ±HHMM
        let m: u32 = s[3..5].parse().ok()?;
        if m > 59 { return None; }
        Some(5)
    }
}

// ─── Slash Date Validation ───────────────────────────────────────────────────

fn validate_slash(s: &str, fmt: &Format) -> bool {
    // Split on the first space to separate date and optional time.
    let (date_part, time_part) = match s.find(' ') {
        Some(pos) => (&s[..pos], Some(&s[pos + 1..])),
        None      => (s, None),
    };

    // Date must be exactly AA/BB/CCCC  (2 / 2 / 4 digits)
    if date_part.len() != 10
        || date_part.as_bytes()[2] != b'/'
        || date_part.as_bytes()[5] != b'/'
    {
        return false;
    }

    let a:    u32  = match date_part[0..2].parse()  { Ok(v) => v, Err(_) => return false };
    let b:    u32  = match date_part[3..5].parse()  { Ok(v) => v, Err(_) => return false };
    let year: i32  = match date_part[6..10].parse() { Ok(v) => v, Err(_) => return false };

    let (month, day) = match fmt {
        Format::DateSlashUS | Format::DateTimeSlashUS => (a, b),   // mm/dd/yyyy
        Format::DateSlashEU | Format::DateTimeSlashEU => (b, a),   // dd/mm/yyyy
        _ => unreachable!(),
    };

    if NaiveDate::from_ymd_opt(year, month, day).is_none() {
        return false;
    }

    let expects_time = matches!(fmt, Format::DateTimeSlashUS | Format::DateTimeSlashEU);
    match (expects_time, time_part) {
        (true,  Some(t)) => t.len() == 8 && parse_hms(t).is_some(),
        (false, None)    => true,
        _                => false,
    }
}

// ─── Compact Date / Datetime Validation ──────────────────────────────────────

fn validate_compact_date(s: &str) -> bool {
    if s.len() != 8 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    // Safety: we verified exactly 8 ASCII digits above.
    let year:  i32  = s[0..4].parse().unwrap();
    let month: u32  = s[4..6].parse().unwrap();
    let day:   u32  = s[6..8].parse().unwrap();
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
    let h: u32   = time[0..2].parse().unwrap();
    let m: u32   = time[2..4].parse().unwrap();
    let sec: u32 = time[4..6].parse().unwrap();
    h <= 23 && m <= 59 && sec <= 59
}

// ─── Unix Epoch Validation ───────────────────────────────────────────────────
//
// Digit-count ranges are deliberately non-overlapping so that the CSP engine
// can discriminate between epoch units without ambiguity:
//
// | Variant           | Digits  | Approx. date range          |
// |-------------------|---------|-----------------------------|
// | UnixSeconds       | 9–10    | 1973-03-03 … 2286-11-20    |
// | UnixMilliseconds  | 11–13   | 1970     … 2286 (ms)       |
// | UnixMicroseconds  | 14–16   | 1970     … 2286 (µs)       |
// | UnixNanoseconds   | 17–19   | 1677     … 2262 (ns i64)   |
//
// Values with 1–8 digits do NOT match any Unix variant.  8-digit numeric
// strings are handled exclusively by DateCompact (if valid YYYYMMDD).

fn validate_unix(s: &str, fmt: &Format) -> bool {
    let num_str = s.strip_prefix('-').unwrap_or(s);

    if num_str.is_empty() || !num_str.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    // Reject leading zeros (except bare "0", which is < 9 digits anyway).
    if num_str.len() > 1 && num_str.as_bytes()[0] == b'0' {
        return false;
    }

    let digits = num_str.len();
    match fmt {
        Format::UnixSeconds       => (9..=10).contains(&digits),
        Format::UnixMilliseconds  => (11..=13).contains(&digits),
        Format::UnixMicroseconds  => (14..=16).contains(&digits),
        Format::UnixNanoseconds   => (17..=19).contains(&digits),
        _ => unreachable!(),
    }
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
                assert!(!ok, "{:?} should reject {:?} (only {:?} expected)", fmt, value, expected);
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
        assert_only("2024-01-15T10:30:00+0530", Format::Iso8601DateTimeOffset);
    }

    #[test]
    fn iso8601_negative_offset() {
        assert_only("2024-01-15T10:30:00-08:00", Format::Iso8601DateTimeOffset);
    }

    #[test]
    fn iso8601_frac() {
        assert_only("2024-01-15T10:30:00.123456", Format::Iso8601DateTimeFrac);
    }

    #[test]
    fn iso8601_frac_utc() {
        assert_only("2024-01-15T10:30:00.123456Z", Format::Iso8601DateTimeFracUtc);
    }

    #[test]
    fn iso8601_frac_offset() {
        assert_only("2024-01-15T10:30:00.123456+05:30", Format::Iso8601DateTimeFracOffset);
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
        assert_none("2024/01/15");  // slash with yyyy first — not a supported layout
    }
}
