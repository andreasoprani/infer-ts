use super::date::*;

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

impl StandardFormat {
    /// Generate all possible format combinations from component enums.
    /// Many combinations are impossible to parse (e.g., slash dates with `T` separator),
    /// but the validator rejects them naturally. The CSP inference eliminates impossible
    /// formats on the first value anyway, so the performance cost is negligible.
    pub(super) fn all_valid() -> Vec<StandardFormat> {
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
                    if matches!(date, DateFmt::Rfc2822) {
                        s.push(' ');
                    }
                    s.push_str(tz.polars_tz());
                }
                s
            }
        }
    }

    /// Validate a value against this standard format.
    pub(super) fn validates(&self, value: &str) -> bool {
        match self {
            StandardFormat::DateOnly { date } => match date {
                DateFmt::Iso => value.len() == 10 && parse_iso_date(value).is_some(),
                DateFmt::SlashUS => validate_slash_date(value, true),
                DateFmt::SlashEU => validate_slash_date(value, false),
                DateFmt::SlashUSShort => validate_slash_date_short(value, true),
                DateFmt::SlashEUShort => validate_slash_date_short(value, false),
                DateFmt::DotEU => validate_dot_date(value),
                DateFmt::DotEUShort => validate_dot_date_short(value),
                DateFmt::Compact => validate_compact_date(value),
                DateFmt::MonthUS => validate_month_us_date(value),
                DateFmt::MonthUSShort => validate_month_us_date_short(value),
                DateFmt::MonthEU => validate_month_eu_date(value),
                DateFmt::MonthEUShort => validate_month_eu_date_short(value),
                DateFmt::Rfc2822 => false, // RFC 2822 always has time + timezone
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
                    DateFmt::SlashUSShort => match (sep, time, tz) {
                        (Separator::Space, TimeFmt::Hms, None) => {
                            validate_slash_datetime_short(value, true)
                        }
                        (Separator::Space, TimeFmt::Hms12, None) => {
                            validate_slash_datetime_12h_short(value, true, true)
                        }
                        (Separator::Space, TimeFmt::Hms12Compact, None) => {
                            validate_slash_datetime_12h_short(value, true, false)
                        }
                        _ => false,
                    },
                    DateFmt::SlashEUShort => match (sep, time, tz) {
                        (Separator::Space, TimeFmt::Hms, None) => {
                            validate_slash_datetime_short(value, false)
                        }
                        (Separator::Space, TimeFmt::Hms12, None) => {
                            validate_slash_datetime_12h_short(value, false, true)
                        }
                        (Separator::Space, TimeFmt::Hms12Compact, None) => {
                            validate_slash_datetime_12h_short(value, false, false)
                        }
                        _ => false,
                    },
                    DateFmt::DotEU => match (sep, time, tz) {
                        (Separator::Space, TimeFmt::Hms, None) => {
                            validate_dot_datetime(value)
                        }
                        (Separator::Space, TimeFmt::Hms12, None) => {
                            validate_dot_datetime_12h(value, true)
                        }
                        (Separator::Space, TimeFmt::Hms12Compact, None) => {
                            validate_dot_datetime_12h(value, false)
                        }
                        _ => false,
                    },
                    DateFmt::DotEUShort => match (sep, time, tz) {
                        (Separator::Space, TimeFmt::Hms, None) => {
                            validate_dot_datetime_short(value)
                        }
                        (Separator::Space, TimeFmt::Hms12, None) => {
                            validate_dot_datetime_12h_short(value, true)
                        }
                        (Separator::Space, TimeFmt::Hms12Compact, None) => {
                            validate_dot_datetime_12h_short(value, false)
                        }
                        _ => false,
                    },
                    DateFmt::Compact => {
                        // Only valid with T separator, compact time, no timezone
                        matches!((sep, time, tz), (Separator::T, TimeFmt::HmsCompact, None))
                            && validate_compact_datetime(value)
                    }
                    DateFmt::MonthUS => match (sep, time, tz) {
                        (Separator::Space, TimeFmt::Hms, None) => {
                            validate_month_us_datetime(value)
                        }
                        (Separator::Space, TimeFmt::Hms12, None) => {
                            validate_month_us_datetime_12h(value, true)
                        }
                        (Separator::Space, TimeFmt::Hms12Compact, None) => {
                            validate_month_us_datetime_12h(value, false)
                        }
                        _ => false,
                    },
                    DateFmt::MonthUSShort => match (sep, time, tz) {
                        (Separator::Space, TimeFmt::Hms, None) => {
                            validate_month_us_datetime_short(value)
                        }
                        (Separator::Space, TimeFmt::Hms12, None) => {
                            validate_month_us_datetime_12h_short(value, true)
                        }
                        (Separator::Space, TimeFmt::Hms12Compact, None) => {
                            validate_month_us_datetime_12h_short(value, false)
                        }
                        _ => false,
                    },
                    DateFmt::MonthEU => match (sep, time, tz) {
                        (Separator::Space, TimeFmt::Hms, None) => {
                            validate_month_eu_datetime(value)
                        }
                        (Separator::Space, TimeFmt::Hms12, None) => {
                            validate_month_eu_datetime_12h(value, true)
                        }
                        (Separator::Space, TimeFmt::Hms12Compact, None) => {
                            validate_month_eu_datetime_12h(value, false)
                        }
                        _ => false,
                    },
                    DateFmt::MonthEUShort => match (sep, time, tz) {
                        (Separator::Space, TimeFmt::Hms, None) => {
                            validate_month_eu_datetime_short(value)
                        }
                        (Separator::Space, TimeFmt::Hms12, None) => {
                            validate_month_eu_datetime_12h_short(value, true)
                        }
                        (Separator::Space, TimeFmt::Hms12Compact, None) => {
                            validate_month_eu_datetime_12h_short(value, false)
                        }
                        _ => false,
                    },
                    DateFmt::Rfc2822 => match (sep, time, tz) {
                        (Separator::Space, TimeFmt::Hms, Some(Timezone::Offset)) => {
                            validate_rfc2822(value, true)
                        }
                        (Separator::Space, TimeFmt::Hms, Some(Timezone::OffsetCompact)) => {
                            validate_rfc2822(value, false)
                        }
                        _ => false,
                    },
                }
            }
        }
    }
}
