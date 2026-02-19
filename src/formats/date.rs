use chrono::NaiveDate;

// ─── Date Format Enum ───────────────────────────────────────────────────────

/// Date format within a standard timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

// ─── DateFormat ─────────────────────────────────────────────────────────────

/// A date-only format (no time component).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DateFormat {
    pub date: DateFmt,
}

impl DateFormat {
    /// Parse a value and return all matching `DateFormat`s.
    pub fn parse(value: &str) -> Vec<DateFormat> {
        let mut matches = Vec::new();
        for date_fmt in DateFmt::all() {
            if let Some(remaining) = parse_date(value, *date_fmt) {
                if remaining.is_empty() {
                    matches.push(DateFormat { date: *date_fmt });
                }
            }
        }
        matches
    }

    /// Validate a value against this specific date format.
    pub(super) fn validates(&self, value: &str) -> bool {
        parse_date(value, self.date).is_some_and(|r| r.is_empty())
    }

    /// Polars-compatible format string.
    pub fn polars_format(&self) -> String {
        self.date.polars_date().to_string()
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

// ─── Component Parser ───────────────────────────────────────────────────────

/// Parse a date from the start of `s` for the given format.
/// Returns `(date, remaining)` on success.
pub(super) fn parse_date_value(s: &str, fmt: DateFmt) -> Option<(NaiveDate, &str)> {
    match fmt {
        DateFmt::Iso => {
            if s.len() < 10 {
                return None;
            }
            let date = parse_iso_date(&s[..10])?;
            Some((date, &s[10..]))
        }
        DateFmt::SlashUS => {
            if s.len() < 10 {
                return None;
            }
            let (a, b, year) = parse_slash_date_parts(&s[..10])?;
            let date = NaiveDate::from_ymd_opt(year, a, b)?;
            Some((date, &s[10..]))
        }
        DateFmt::SlashEU => {
            if s.len() < 10 {
                return None;
            }
            let (a, b, year) = parse_slash_date_parts(&s[..10])?;
            let date = NaiveDate::from_ymd_opt(year, b, a)?;
            Some((date, &s[10..]))
        }
        DateFmt::SlashUSShort => {
            if s.len() < 8 {
                return None;
            }
            let (a, b, year) = parse_slash_date_parts_short(&s[..8])?;
            let date = NaiveDate::from_ymd_opt(year, a, b)?;
            Some((date, &s[8..]))
        }
        DateFmt::SlashEUShort => {
            if s.len() < 8 {
                return None;
            }
            let (a, b, year) = parse_slash_date_parts_short(&s[..8])?;
            let date = NaiveDate::from_ymd_opt(year, b, a)?;
            Some((date, &s[8..]))
        }
        DateFmt::DotEU => {
            if s.len() < 10 {
                return None;
            }
            let (day, month, year) = parse_dot_date_parts(&s[..10])?;
            let date = NaiveDate::from_ymd_opt(year, month, day)?;
            Some((date, &s[10..]))
        }
        DateFmt::DotEUShort => {
            if s.len() < 8 {
                return None;
            }
            let (day, month, year) = parse_dot_date_parts_short(&s[..8])?;
            let date = NaiveDate::from_ymd_opt(year, month, day)?;
            Some((date, &s[8..]))
        }
        DateFmt::Compact => {
            if s.len() < 8 {
                return None;
            }
            if !validate_compact_date(&s[..8]) {
                return None;
            }
            let year: i32 = s[0..4].parse().ok()?;
            let month: u32 = s[4..6].parse().ok()?;
            let day: u32 = s[6..8].parse().ok()?;
            let date = NaiveDate::from_ymd_opt(year, month, day)?;
            Some((date, &s[8..]))
        }
        DateFmt::MonthUS => {
            let (day, month, year, consumed) = parse_month_us_date_parts(s)?;
            let date = NaiveDate::from_ymd_opt(year, month, day)?;
            Some((date, &s[consumed..]))
        }
        DateFmt::MonthUSShort => {
            let (day, month, year, consumed) = parse_month_us_date_parts_short(s)?;
            let date = NaiveDate::from_ymd_opt(year, month, day)?;
            Some((date, &s[consumed..]))
        }
        DateFmt::MonthEU => {
            let (day, month, year, consumed) = parse_month_eu_date_parts(s)?;
            let date = NaiveDate::from_ymd_opt(year, month, day)?;
            Some((date, &s[consumed..]))
        }
        DateFmt::MonthEUShort => {
            let (day, month, year, consumed) = parse_month_eu_date_parts_short(s)?;
            let date = NaiveDate::from_ymd_opt(year, month, day)?;
            Some((date, &s[consumed..]))
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
            Some((date, &s[5 + consumed..]))
        }
    }
}

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
