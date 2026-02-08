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
    /// `MM/DD/YY` (US convention, 2-digit year)
    SlashUSShort,
    /// `DD/MM/YY` (EU convention, 2-digit year)
    SlashEUShort,
    /// `DD.MM.YYYY` (European convention, dot-separated)
    DotEU,
    /// `DD.MM.YY` (European convention, dot-separated, 2-digit year)
    DotEUShort,
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

// ─── Component Fragments ────────────────────────────────────────────────────
//
// Each component knows how to contribute its fragment to a Polars format string.
// `StandardFormat::all_valid()` generates all possible combinations; the validator
// rejects impossible ones. Adding a new variant only requires implementing the
// fragment method and ensuring `validates()` handles it.

impl DateFmt {
    /// All date formats, in definition order.
    pub(super) const fn all() -> &'static [DateFmt] {
        &[
            DateFmt::Iso,
            DateFmt::SlashUS,
            DateFmt::SlashEU,
            DateFmt::SlashUSShort,
            DateFmt::SlashEUShort,
            DateFmt::DotEU,
            DateFmt::DotEUShort,
            DateFmt::Compact,
        ]
    }

    /// Polars date fragment.
    pub(super) fn polars_date(&self) -> &'static str {
        match self {
            DateFmt::Iso => "%Y-%m-%d",
            DateFmt::SlashUS => "%m/%d/%Y",
            DateFmt::SlashEU => "%d/%m/%Y",
            DateFmt::SlashUSShort => "%m/%d/%y",
            DateFmt::SlashEUShort => "%d/%m/%y",
            DateFmt::DotEU => "%d.%m.%Y",
            DateFmt::DotEUShort => "%d.%m.%y",
            DateFmt::Compact => "%Y%m%d",
        }
    }
}

impl Separator {
    /// Polars separator fragment.
    pub(super) fn polars_sep(&self) -> &'static str {
        match self {
            Separator::T => "T",
            Separator::Space => " ",
        }
    }
}

impl TimeFmt {
    /// Polars time fragment.
    pub(super) fn polars_time(&self) -> &'static str {
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
    pub(super) fn polars_tz(&self) -> &'static str {
        match self {
            Timezone::Utc => "Z",
            Timezone::Offset => "%:z",
            Timezone::OffsetCompact => "%z",
        }
    }
}

// ─── Shared Parsing Primitives ───────────────────────────────────────────────

/// Structural description of a parsed ISO-like timestamp.
/// Internal type used during validation.
pub(super) struct IsoStructure {
    pub(super) separator: ParsedSeparator,
    pub(super) has_frac: bool,
    pub(super) suffix: ParsedSuffix,
}

/// Separator parsed from input (internal).
#[derive(PartialEq)]
pub(super) enum ParsedSeparator {
    T,
    Space,
}

/// Timezone suffix parsed from input (internal).
#[derive(PartialEq)]
pub(super) enum ParsedSuffix {
    None,
    UtcZ,
    /// Offset with colon: `+05:30` or `-08:00`
    OffsetColon,
    /// Compact offset without colon: `+0530` or `-0800`
    OffsetCompact,
}

/// Parse a `YYYY-MM-DD[T| ]HH:MM:SS[.f…][Z|±HH:MM]` string and return its
/// structural components.  Returns `None` on any syntax error.
pub(super) fn parse_iso_like(s: &str) -> Option<IsoStructure> {
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
pub(super) fn parse_iso_date(s: &str) -> Option<NaiveDate> {
    if s.len() != 10 || s.as_bytes()[4] != b'-' || s.as_bytes()[7] != b'-' {
        return None;
    }
    let year: i32 = s[0..4].parse().ok()?;
    let month: u32 = s[5..7].parse().ok()?;
    let day: u32 = s[8..10].parse().ok()?;
    NaiveDate::from_ymd_opt(year, month, day)
}

/// Validate an `HH:MM:SS` string (exactly 8 bytes).
pub(super) fn parse_hms(s: &str) -> Option<()> {
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
pub(super) fn parse_tz_offset(s: &str) -> Option<(usize, bool)> {
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
pub(super) fn parse_slash_date_parts(s: &str) -> Option<(u32, u32, i32)> {
    if s.len() != 10 || s.as_bytes()[2] != b'/' || s.as_bytes()[5] != b'/' {
        return None;
    }
    let a: u32 = s[0..2].parse().ok()?;
    let b: u32 = s[3..5].parse().ok()?;
    let year: i32 = s[6..10].parse().ok()?;
    Some((a, b, year))
}

/// Validate a slash date (no time). `is_us` determines mm/dd/yyyy vs dd/mm/yyyy.
pub(super) fn validate_slash_date(s: &str, is_us: bool) -> bool {
    let Some((a, b, year)) = parse_slash_date_parts(s) else {
        return false;
    };
    let (month, day) = if is_us { (a, b) } else { (b, a) };
    NaiveDate::from_ymd_opt(year, month, day).is_some()
}

/// Validate a slash datetime. `is_us` determines mm/dd/yyyy vs dd/mm/yyyy.
pub(super) fn validate_slash_datetime(s: &str, is_us: bool) -> bool {
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
pub(super) fn parse_hms12(s: &str, space_before_ampm: bool) -> Option<usize> {
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
pub(super) fn validate_slash_datetime_12h(s: &str, is_us: bool, space_before_ampm: bool) -> bool {
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

// ─── Short (2-digit year) Slash Date Validation ─────────────────────────────

/// Expand a 2-digit year to 4-digit using POSIX convention: 00–68 → 2000–2068, 69–99 → 1969–1999.
fn expand_year(yy: i32) -> i32 {
    if yy <= 68 { 2000 + yy } else { 1900 + yy }
}

/// Parse slash date parts (AA/BB/CC) for 2-digit year and return (a, b, expanded_year).
pub(super) fn parse_slash_date_parts_short(s: &str) -> Option<(u32, u32, i32)> {
    if s.len() != 8 || s.as_bytes()[2] != b'/' || s.as_bytes()[5] != b'/' {
        return None;
    }
    let a: u32 = s[0..2].parse().ok()?;
    let b: u32 = s[3..5].parse().ok()?;
    let yy: i32 = s[6..8].parse().ok()?;
    Some((a, b, expand_year(yy)))
}

/// Validate a short-year slash date (no time). `is_us` determines mm/dd/yy vs dd/mm/yy.
pub(super) fn validate_slash_date_short(s: &str, is_us: bool) -> bool {
    let Some((a, b, year)) = parse_slash_date_parts_short(s) else {
        return false;
    };
    let (month, day) = if is_us { (a, b) } else { (b, a) };
    NaiveDate::from_ymd_opt(year, month, day).is_some()
}

/// Validate a short-year slash datetime. `is_us` determines mm/dd/yy vs dd/mm/yy.
pub(super) fn validate_slash_datetime_short(s: &str, is_us: bool) -> bool {
    let Some(space_pos) = s.find(' ') else {
        return false;
    };
    let date_part = &s[..space_pos];
    let time_part = &s[space_pos + 1..];

    let Some((a, b, year)) = parse_slash_date_parts_short(date_part) else {
        return false;
    };
    let (month, day) = if is_us { (a, b) } else { (b, a) };

    if NaiveDate::from_ymd_opt(year, month, day).is_none() {
        return false;
    }

    time_part.len() == 8 && parse_hms(time_part).is_some()
}

/// Validate a short-year slash datetime with 12-hour time.
pub(super) fn validate_slash_datetime_12h_short(
    s: &str,
    is_us: bool,
    space_before_ampm: bool,
) -> bool {
    let Some(space_pos) = s.find(' ') else {
        return false;
    };
    let date_part = &s[..space_pos];
    let time_part = &s[space_pos + 1..];

    let Some((a, b, year)) = parse_slash_date_parts_short(date_part) else {
        return false;
    };
    let (month, day) = if is_us { (a, b) } else { (b, a) };

    if NaiveDate::from_ymd_opt(year, month, day).is_none() {
        return false;
    }

    parse_hms12(time_part, space_before_ampm) == Some(time_part.len())
}

// ─── Dot-Separated Date Validation (EU only) ────────────────────────────────

/// Parse dot date parts (DD.MM.YYYY) and return (day, month, year) if valid structure.
pub(super) fn parse_dot_date_parts(s: &str) -> Option<(u32, u32, i32)> {
    if s.len() != 10 || s.as_bytes()[2] != b'.' || s.as_bytes()[5] != b'.' {
        return None;
    }
    let day: u32 = s[0..2].parse().ok()?;
    let month: u32 = s[3..5].parse().ok()?;
    let year: i32 = s[6..10].parse().ok()?;
    Some((day, month, year))
}

/// Parse dot date parts (DD.MM.YY) for 2-digit year and return (day, month, expanded_year).
pub(super) fn parse_dot_date_parts_short(s: &str) -> Option<(u32, u32, i32)> {
    if s.len() != 8 || s.as_bytes()[2] != b'.' || s.as_bytes()[5] != b'.' {
        return None;
    }
    let day: u32 = s[0..2].parse().ok()?;
    let month: u32 = s[3..5].parse().ok()?;
    let yy: i32 = s[6..8].parse().ok()?;
    Some((day, month, expand_year(yy)))
}

/// Validate a dot date (no time). Always DD.MM.YYYY (EU only).
pub(super) fn validate_dot_date(s: &str) -> bool {
    let Some((day, month, year)) = parse_dot_date_parts(s) else {
        return false;
    };
    NaiveDate::from_ymd_opt(year, month, day).is_some()
}

/// Validate a short-year dot date (no time). Always DD.MM.YY (EU only).
pub(super) fn validate_dot_date_short(s: &str) -> bool {
    let Some((day, month, year)) = parse_dot_date_parts_short(s) else {
        return false;
    };
    NaiveDate::from_ymd_opt(year, month, day).is_some()
}

/// Validate a dot datetime. Always DD.MM.YYYY HH:MM:SS (EU only).
pub(super) fn validate_dot_datetime(s: &str) -> bool {
    let Some(space_pos) = s.find(' ') else {
        return false;
    };
    let date_part = &s[..space_pos];
    let time_part = &s[space_pos + 1..];

    let Some((day, month, year)) = parse_dot_date_parts(date_part) else {
        return false;
    };

    if NaiveDate::from_ymd_opt(year, month, day).is_none() {
        return false;
    }

    time_part.len() == 8 && parse_hms(time_part).is_some()
}

/// Validate a short-year dot datetime. Always DD.MM.YY HH:MM:SS (EU only).
pub(super) fn validate_dot_datetime_short(s: &str) -> bool {
    let Some(space_pos) = s.find(' ') else {
        return false;
    };
    let date_part = &s[..space_pos];
    let time_part = &s[space_pos + 1..];

    let Some((day, month, year)) = parse_dot_date_parts_short(date_part) else {
        return false;
    };

    if NaiveDate::from_ymd_opt(year, month, day).is_none() {
        return false;
    }

    time_part.len() == 8 && parse_hms(time_part).is_some()
}

/// Validate a dot datetime with 12-hour time. Always DD.MM.YYYY (EU only).
/// `space_before_ampm` controls whether a space is expected before AM/PM.
pub(super) fn validate_dot_datetime_12h(s: &str, space_before_ampm: bool) -> bool {
    let Some(space_pos) = s.find(' ') else {
        return false;
    };
    let date_part = &s[..space_pos];
    let time_part = &s[space_pos + 1..];

    let Some((day, month, year)) = parse_dot_date_parts(date_part) else {
        return false;
    };

    if NaiveDate::from_ymd_opt(year, month, day).is_none() {
        return false;
    }

    parse_hms12(time_part, space_before_ampm) == Some(time_part.len())
}

/// Validate a short-year dot datetime with 12-hour time. Always DD.MM.YY (EU only).
pub(super) fn validate_dot_datetime_12h_short(s: &str, space_before_ampm: bool) -> bool {
    let Some(space_pos) = s.find(' ') else {
        return false;
    };
    let date_part = &s[..space_pos];
    let time_part = &s[space_pos + 1..];

    let Some((day, month, year)) = parse_dot_date_parts_short(date_part) else {
        return false;
    };

    if NaiveDate::from_ymd_opt(year, month, day).is_none() {
        return false;
    }

    parse_hms12(time_part, space_before_ampm) == Some(time_part.len())
}

// ─── Compact Date / Datetime Validation ──────────────────────────────────────

pub(super) fn validate_compact_date(s: &str) -> bool {
    if s.len() != 8 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    // Safety: we verified exactly 8 ASCII digits above.
    let year: i32 = s[0..4].parse().unwrap();
    let month: u32 = s[4..6].parse().unwrap();
    let day: u32 = s[6..8].parse().unwrap();
    NaiveDate::from_ymd_opt(year, month, day).is_some()
}

pub(super) fn validate_compact_datetime(s: &str) -> bool {
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
