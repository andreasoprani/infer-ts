// ─── Time Component Enums ───────────────────────────────────────────────────

/// Separator between date and time components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Separator {
    /// `T` or `t` (ISO 8601)
    T,
    /// Single space
    Space,
}

/// Time format within a standard timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Timezone {
    /// `Z` or `z` (UTC)
    Utc,
    /// `±HH:MM` (with colon)
    Offset,
    /// `±HHMM` (compact, no colon)
    OffsetCompact,
}

// ─── Polars Fragments ───────────────────────────────────────────────────────

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

// ─── Component Parsers ──────────────────────────────────────────────────────

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

#[cfg(test)]
impl TimeFmt {
    /// All time formats, in definition order.
    pub(super) const fn all() -> &'static [TimeFmt] {
        &[
            TimeFmt::Hms,
            TimeFmt::HmsFrac,
            TimeFmt::HmsCompact,
            TimeFmt::Hms12,
            TimeFmt::Hms12Compact,
        ]
    }

    /// Human-readable name for documentation tables.
    pub(super) fn name(&self) -> &'static str {
        match self {
            TimeFmt::Hms => "24-hour",
            TimeFmt::HmsFrac => "24-hour + fractional seconds",
            TimeFmt::HmsCompact => "24-hour compact",
            TimeFmt::Hms12 => "12-hour AM/PM",
            TimeFmt::Hms12Compact => "12-hour AM/PM compact",
        }
    }

    /// Canonical example value for documentation tables.
    pub(super) fn example(&self) -> &'static str {
        match self {
            TimeFmt::Hms => "10:30:00",
            TimeFmt::HmsFrac => "10:30:00.123456",
            TimeFmt::HmsCompact => "103000",
            TimeFmt::Hms12 => "10:30:00 PM",
            TimeFmt::Hms12Compact => "10:30:00PM",
        }
    }
}

#[cfg(test)]
impl Timezone {
    /// All timezone variants, in definition order.
    pub(super) const fn all() -> &'static [Timezone] {
        &[Timezone::Utc, Timezone::Offset, Timezone::OffsetCompact]
    }

    /// Human-readable name for documentation tables.
    pub(super) fn name(&self) -> &'static str {
        match self {
            Timezone::Utc => "UTC",
            Timezone::Offset => "Offset with colon",
            Timezone::OffsetCompact => "Compact offset",
        }
    }

    /// Canonical example value for documentation tables.
    pub(super) fn example(&self) -> &'static str {
        match self {
            Timezone::Utc => "Z",
            Timezone::Offset => "+05:30",
            Timezone::OffsetCompact => "+0530",
        }
    }
}
