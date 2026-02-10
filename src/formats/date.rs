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
    /// `Jan 15, 2024` (US convention, month name)
    MonthUS,
    /// `Jan 15, 24` (US convention, month name, 2-digit year)
    MonthUSShort,
    /// `15 Jan 2024` (EU convention, month name)
    MonthEU,
    /// `15 Jan 24` (EU convention, month name, 2-digit year)
    MonthEUShort,
    /// `Mon, 15 Jan 2024 10:30:00 +0530` (RFC 2822 / email style)
    Rfc2822,
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
    /// `HHMMSS` (compact, no colons)
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
            DateFmt::MonthUS,
            DateFmt::MonthUSShort,
            DateFmt::MonthEU,
            DateFmt::MonthEUShort,
            DateFmt::Rfc2822,
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
            DateFmt::MonthUS => "%b %d, %Y",
            DateFmt::MonthUSShort => "%b %d, %y",
            DateFmt::MonthEU => "%d %b %Y",
            DateFmt::MonthEUShort => "%d %b %y",
            DateFmt::Rfc2822 => "%a, %d %b %Y",
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

// ─── Parsing Primitives ─────────────────────────────────────────────────────

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
/// Returns `(bytes_consumed, has_colon)`.
fn parse_tz_offset(s: &str) -> Option<(usize, bool)> {
    if s.len() < 5 || (s.as_bytes()[0] != b'+' && s.as_bytes()[0] != b'-') {
        return None;
    }
    let h: u32 = s[1..3].parse().ok()?;
    if h > 23 {
        return None;
    }

    if s.len() >= 6 && s.as_bytes()[3] == b':' {
        let m: u32 = s[4..6].parse().ok()?;
        if m > 59 {
            return None;
        }
        Some((6, true))
    } else {
        let m: u32 = s[3..5].parse().ok()?;
        if m > 59 {
            return None;
        }
        Some((5, false))
    }
}

/// Parse slash date parts (AA/BB/CCCC) and return (a, b, year).
fn parse_slash_date_parts(s: &str) -> Option<(u32, u32, i32)> {
    if s.len() != 10 || s.as_bytes()[2] != b'/' || s.as_bytes()[5] != b'/' {
        return None;
    }
    let a: u32 = s[0..2].parse().ok()?;
    let b: u32 = s[3..5].parse().ok()?;
    let year: i32 = s[6..10].parse().ok()?;
    Some((a, b, year))
}

/// Expand a 2-digit year to 4-digit using POSIX convention: 00–68 → 2000–2068, 69–99 → 1969–1999.
fn expand_year(yy: i32) -> i32 {
    if yy <= 68 {
        2000 + yy
    } else {
        1900 + yy
    }
}

/// Parse slash date parts (AA/BB/CC) for 2-digit year and return (a, b, expanded_year).
fn parse_slash_date_parts_short(s: &str) -> Option<(u32, u32, i32)> {
    if s.len() != 8 || s.as_bytes()[2] != b'/' || s.as_bytes()[5] != b'/' {
        return None;
    }
    let a: u32 = s[0..2].parse().ok()?;
    let b: u32 = s[3..5].parse().ok()?;
    let yy: i32 = s[6..8].parse().ok()?;
    Some((a, b, expand_year(yy)))
}

/// Parse dot date parts (DD.MM.YYYY) and return (day, month, year).
fn parse_dot_date_parts(s: &str) -> Option<(u32, u32, i32)> {
    if s.len() != 10 || s.as_bytes()[2] != b'.' || s.as_bytes()[5] != b'.' {
        return None;
    }
    let day: u32 = s[0..2].parse().ok()?;
    let month: u32 = s[3..5].parse().ok()?;
    let year: i32 = s[6..10].parse().ok()?;
    Some((day, month, year))
}

/// Parse dot date parts (DD.MM.YY) for 2-digit year.
fn parse_dot_date_parts_short(s: &str) -> Option<(u32, u32, i32)> {
    if s.len() != 8 || s.as_bytes()[2] != b'.' || s.as_bytes()[5] != b'.' {
        return None;
    }
    let day: u32 = s[0..2].parse().ok()?;
    let month: u32 = s[3..5].parse().ok()?;
    let yy: i32 = s[6..8].parse().ok()?;
    Some((day, month, expand_year(yy)))
}

/// Validate compact date YYYYMMDD (exactly 8 ASCII digits).
fn validate_compact_date(s: &str) -> bool {
    if s.len() != 8 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let year: i32 = s[0..4].parse().unwrap();
    let month: u32 = s[4..6].parse().unwrap();
    let day: u32 = s[6..8].parse().unwrap();
    NaiveDate::from_ymd_opt(year, month, day).is_some()
}

/// Parse a 3-character month abbreviation (case-insensitive) to a 1-based month number.
fn parse_month_abbr(s: &str) -> Option<u32> {
    if s.len() < 3 {
        return None;
    }
    match s[..3].to_ascii_lowercase().as_str() {
        "jan" => Some(1),
        "feb" => Some(2),
        "mar" => Some(3),
        "apr" => Some(4),
        "may" => Some(5),
        "jun" => Some(6),
        "jul" => Some(7),
        "aug" => Some(8),
        "sep" => Some(9),
        "oct" => Some(10),
        "nov" => Some(11),
        "dec" => Some(12),
        _ => None,
    }
}

/// Parse US month-name date `Jan DD, YYYY` and return (day, month, year, bytes_consumed).
fn parse_month_us_date_parts(s: &str) -> Option<(u32, u32, i32, usize)> {
    if s.len() < 11 {
        return None;
    }
    let month = parse_month_abbr(s)?;
    if s.as_bytes()[3] != b' ' {
        return None;
    }
    let comma = s[4..].find(',')?;
    let comma_pos = 4 + comma;
    let day: u32 = s[4..comma_pos].parse().ok()?;
    if comma_pos + 1 >= s.len() || s.as_bytes()[comma_pos + 1] != b' ' {
        return None;
    }
    let year_start = comma_pos + 2;
    if year_start + 4 > s.len() {
        return None;
    }
    let year_str = &s[year_start..year_start + 4];
    if !year_str.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let year: i32 = year_str.parse().ok()?;
    Some((day, month, year, year_start + 4))
}

/// Parse US month-name date with 2-digit year `Jan DD, YY`.
fn parse_month_us_date_parts_short(s: &str) -> Option<(u32, u32, i32, usize)> {
    if s.len() < 9 {
        return None;
    }
    let month = parse_month_abbr(s)?;
    if s.as_bytes()[3] != b' ' {
        return None;
    }
    let comma = s[4..].find(',')?;
    let comma_pos = 4 + comma;
    let day: u32 = s[4..comma_pos].parse().ok()?;
    if comma_pos + 1 >= s.len() || s.as_bytes()[comma_pos + 1] != b' ' {
        return None;
    }
    let year_start = comma_pos + 2;
    if year_start + 2 > s.len() {
        return None;
    }
    let year_str = &s[year_start..year_start + 2];
    if !year_str.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let yy: i32 = year_str.parse().ok()?;
    Some((day, month, expand_year(yy), year_start + 2))
}

/// Parse EU month-name date `DD Mon YYYY` and return (day, month, year, bytes_consumed).
fn parse_month_eu_date_parts(s: &str) -> Option<(u32, u32, i32, usize)> {
    if s.len() < 10 {
        return None;
    }
    let space1 = s.find(' ')?;
    if space1 == 0 || space1 > 2 {
        return None;
    }
    let day: u32 = s[..space1].parse().ok()?;
    let month_start = space1 + 1;
    if month_start + 3 > s.len() {
        return None;
    }
    let month = parse_month_abbr(&s[month_start..])?;
    let after_month = month_start + 3;
    if after_month >= s.len() || s.as_bytes()[after_month] != b' ' {
        return None;
    }
    let year_start = after_month + 1;
    if year_start + 4 > s.len() {
        return None;
    }
    let year_str = &s[year_start..year_start + 4];
    if !year_str.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let year: i32 = year_str.parse().ok()?;
    Some((day, month, year, year_start + 4))
}

/// Parse EU month-name date with 2-digit year `DD Mon YY`.
fn parse_month_eu_date_parts_short(s: &str) -> Option<(u32, u32, i32, usize)> {
    if s.len() < 8 {
        return None;
    }
    let space1 = s.find(' ')?;
    if space1 == 0 || space1 > 2 {
        return None;
    }
    let day: u32 = s[..space1].parse().ok()?;
    let month_start = space1 + 1;
    if month_start + 3 > s.len() {
        return None;
    }
    let month = parse_month_abbr(&s[month_start..])?;
    let after_month = month_start + 3;
    if after_month >= s.len() || s.as_bytes()[after_month] != b' ' {
        return None;
    }
    let year_start = after_month + 1;
    if year_start + 2 > s.len() {
        return None;
    }
    let year_str = &s[year_start..year_start + 2];
    if !year_str.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let yy: i32 = year_str.parse().ok()?;
    Some((day, month, expand_year(yy), year_start + 2))
}

/// Parse a 12-hour time with AM/PM suffix (case-insensitive).
///
/// When `space_before_ampm` is `true`, expects `H:MM:SS AM` / `HH:MM:SS PM`.
/// When `false`, expects `H:MM:SSAM` / `HH:MM:SSPM` (no space).
///
/// Returns the number of bytes consumed on success, or `None` on failure.
fn parse_hms12(s: &str, space_before_ampm: bool) -> Option<usize> {
    let b = s.as_bytes();
    let colon1 = if b.len() > 1 && b[1] == b':' {
        1
    } else if b.len() > 2 && b[2] == b':' {
        2
    } else {
        return None;
    };

    let h: u32 = s[..colon1].parse().ok()?;
    if !(1..=12).contains(&h) {
        return None;
    }

    let rest = &s[colon1..];
    if rest.len() < 6 || rest.as_bytes()[0] != b':' || rest.as_bytes()[3] != b':' {
        return None;
    }
    let m: u32 = rest[1..3].parse().ok()?;
    let sec: u32 = rest[4..6].parse().ok()?;
    if m > 59 || sec > 59 {
        return None;
    }

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

/// Parse a 3-character weekday abbreviation (case-insensitive) to a `chrono::Weekday`.
fn parse_dow_abbr(s: &str) -> Option<chrono::Weekday> {
    use chrono::Weekday::*;
    if s.len() < 3 {
        return None;
    }
    match s[..3].to_ascii_lowercase().as_str() {
        "mon" => Some(Mon),
        "tue" => Some(Tue),
        "wed" => Some(Wed),
        "thu" => Some(Thu),
        "fri" => Some(Fri),
        "sat" => Some(Sat),
        "sun" => Some(Sun),
        _ => None,
    }
}

// ─── Component Parsers ──────────────────────────────────────────────────────

/// Parse a date from the start of `s` for the given format.
/// Returns the remaining (unconsumed) string on success.
pub(super) fn parse_date(s: &str, fmt: DateFmt) -> Option<&str> {
    match fmt {
        DateFmt::Iso => {
            if s.len() < 10 {
                return None;
            }
            parse_iso_date(&s[..10])?;
            Some(&s[10..])
        }
        DateFmt::SlashUS => {
            if s.len() < 10 {
                return None;
            }
            let (a, b, year) = parse_slash_date_parts(&s[..10])?;
            NaiveDate::from_ymd_opt(year, a, b)?;
            Some(&s[10..])
        }
        DateFmt::SlashEU => {
            if s.len() < 10 {
                return None;
            }
            let (a, b, year) = parse_slash_date_parts(&s[..10])?;
            NaiveDate::from_ymd_opt(year, b, a)?;
            Some(&s[10..])
        }
        DateFmt::SlashUSShort => {
            if s.len() < 8 {
                return None;
            }
            let (a, b, year) = parse_slash_date_parts_short(&s[..8])?;
            NaiveDate::from_ymd_opt(year, a, b)?;
            Some(&s[8..])
        }
        DateFmt::SlashEUShort => {
            if s.len() < 8 {
                return None;
            }
            let (a, b, year) = parse_slash_date_parts_short(&s[..8])?;
            NaiveDate::from_ymd_opt(year, b, a)?;
            Some(&s[8..])
        }
        DateFmt::DotEU => {
            if s.len() < 10 {
                return None;
            }
            let (day, month, year) = parse_dot_date_parts(&s[..10])?;
            NaiveDate::from_ymd_opt(year, month, day)?;
            Some(&s[10..])
        }
        DateFmt::DotEUShort => {
            if s.len() < 8 {
                return None;
            }
            let (day, month, year) = parse_dot_date_parts_short(&s[..8])?;
            NaiveDate::from_ymd_opt(year, month, day)?;
            Some(&s[8..])
        }
        DateFmt::Compact => {
            if s.len() < 8 {
                return None;
            }
            if !validate_compact_date(&s[..8]) {
                return None;
            }
            Some(&s[8..])
        }
        DateFmt::MonthUS => {
            let (day, month, year, consumed) = parse_month_us_date_parts(s)?;
            NaiveDate::from_ymd_opt(year, month, day)?;
            Some(&s[consumed..])
        }
        DateFmt::MonthUSShort => {
            let (day, month, year, consumed) = parse_month_us_date_parts_short(s)?;
            NaiveDate::from_ymd_opt(year, month, day)?;
            Some(&s[consumed..])
        }
        DateFmt::MonthEU => {
            let (day, month, year, consumed) = parse_month_eu_date_parts(s)?;
            NaiveDate::from_ymd_opt(year, month, day)?;
            Some(&s[consumed..])
        }
        DateFmt::MonthEUShort => {
            let (day, month, year, consumed) = parse_month_eu_date_parts_short(s)?;
            NaiveDate::from_ymd_opt(year, month, day)?;
            Some(&s[consumed..])
        }
        DateFmt::Rfc2822 => {
            if s.len() < 5 {
                return None;
            }
            let dow = parse_dow_abbr(s)?;
            if s.as_bytes()[3] != b',' || s.as_bytes()[4] != b' ' {
                return None;
            }
            let (day, month, year, consumed) = parse_month_eu_date_parts(&s[5..])?;
            let date = NaiveDate::from_ymd_opt(year, month, day)?;
            use chrono::Datelike;
            if date.weekday() != dow {
                return None;
            }
            Some(&s[5 + consumed..])
        }
    }
}

/// Parse a separator (T/t or space) from the start of `s`.
pub(super) fn parse_separator(s: &str, sep: Separator) -> Option<&str> {
    if s.is_empty() {
        return None;
    }
    match sep {
        Separator::T => {
            if matches!(s.as_bytes()[0], b'T' | b't') {
                Some(&s[1..])
            } else {
                None
            }
        }
        Separator::Space => {
            if s.as_bytes()[0] == b' ' {
                Some(&s[1..])
            } else {
                None
            }
        }
    }
}

/// Parse fractional seconds (`.` + 1-9 digits) from the start of `s`.
/// Returns `(frac_digits, remaining)` on success.
pub(super) fn parse_frac(s: &str) -> Option<(usize, &str)> {
    if s.is_empty() || s.as_bytes()[0] != b'.' {
        return None;
    }
    let frac_digits = s[1..].bytes().take_while(|b| b.is_ascii_digit()).count();
    if frac_digits == 0 || frac_digits > 9 {
        return None;
    }
    Some((frac_digits, &s[1 + frac_digits..]))
}

/// Parse a time from the start of `s` for the given format.
/// Returns the remaining (unconsumed) string on success.
pub(super) fn parse_time(s: &str, fmt: TimeFmt) -> Option<&str> {
    match fmt {
        TimeFmt::Hms => {
            if s.len() < 8 {
                return None;
            }
            parse_hms(&s[..8])?;
            // Must not have fractional seconds
            if s.len() > 8 && s.as_bytes()[8] == b'.' {
                return None;
            }
            Some(&s[8..])
        }
        TimeFmt::HmsFrac => {
            if s.len() < 8 {
                return None;
            }
            parse_hms(&s[..8])?;
            let (_, remaining) = parse_frac(&s[8..])?;
            Some(remaining)
        }
        TimeFmt::HmsCompact => {
            if s.len() < 6 {
                return None;
            }
            let time = &s[..6];
            if !time.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let h: u32 = time[0..2].parse().ok()?;
            let m: u32 = time[2..4].parse().ok()?;
            let sec: u32 = time[4..6].parse().ok()?;
            if h > 23 || m > 59 || sec > 59 {
                return None;
            }
            Some(&s[6..])
        }
        TimeFmt::Hms12 => {
            let consumed = parse_hms12(s, true)?;
            Some(&s[consumed..])
        }
        TimeFmt::Hms12Compact => {
            let consumed = parse_hms12(s, false)?;
            Some(&s[consumed..])
        }
    }
}

/// Parse a timezone suffix from the start of `s`.
pub(super) fn parse_timezone(s: &str, tz: Timezone) -> Option<&str> {
    match tz {
        Timezone::Utc => {
            if !s.is_empty() && matches!(s.as_bytes()[0], b'Z' | b'z') {
                Some(&s[1..])
            } else {
                None
            }
        }
        Timezone::Offset => {
            let (n, has_colon) = parse_tz_offset(s)?;
            if has_colon {
                Some(&s[n..])
            } else {
                None
            }
        }
        Timezone::OffsetCompact => {
            let (n, has_colon) = parse_tz_offset(s)?;
            if !has_colon {
                Some(&s[n..])
            } else {
                None
            }
        }
    }
}
