use super::date::*;

/// Time component of a standard datetime: separator + time format + optional timezone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TimeComponent {
    pub separator: Separator,
    pub format: TimeFmt,
    pub timezone: Option<Timezone>,
    /// Whether there is a space between time and timezone (e.g., `10:30:00 +05:30`).
    pub spaced_tz: bool,
}

/// A standard datetime format composed of date + optional time component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StandardFormat {
    /// Date only (no time component)
    DateOnly { date: DateFmt },
    /// Date and time with optional timezone
    DateTime { date: DateFmt, time: TimeComponent },
}

impl StandardFormat {
    /// Parse a value and return all matching `StandardFormat`s.
    ///
    /// Compositional parsing: parse date → separator → time → timezone linearly,
    /// accepting all structurally valid combinations.
    pub fn parse(value: &str) -> Vec<StandardFormat> {
        let mut matches = Vec::new();

        for date_fmt in DateFmt::all() {
            let Some(remaining) = parse_date(value, *date_fmt) else {
                continue;
            };

            if remaining.is_empty() {
                matches.push(StandardFormat::DateOnly { date: *date_fmt });
                continue;
            }

            Self::parse_time_components(remaining, *date_fmt, &mut matches);
        }

        matches
    }

    /// Try all separator × time × timezone combinations on the remaining string
    /// after the date has been consumed.
    fn parse_time_components(
        remaining: &str,
        date_fmt: DateFmt,
        matches: &mut Vec<StandardFormat>,
    ) {
        for sep in [Separator::T, Separator::Space] {
            let Some(after_sep) = parse_separator(remaining, sep) else {
                continue;
            };

            for time_fmt in [
                TimeFmt::Hms,
                TimeFmt::HmsFrac,
                TimeFmt::HmsCompact,
                TimeFmt::Hms12,
                TimeFmt::Hms12Compact,
            ] {
                let Some(after_time) = parse_time(after_sep, time_fmt) else {
                    continue;
                };

                // No timezone
                if after_time.is_empty() {
                    matches.push(StandardFormat::DateTime {
                        date: date_fmt,
                        time: TimeComponent {
                            separator: sep,
                            format: time_fmt,
                            timezone: None,
                            spaced_tz: false,
                        },
                    });
                    continue;
                }

                // Try each timezone variant, with and without space before tz
                for tz in [Timezone::Utc, Timezone::Offset, Timezone::OffsetCompact] {
                    for spaced in [false, true] {
                        let tz_input = if spaced {
                            if after_time.is_empty() || after_time.as_bytes()[0] != b' ' {
                                continue;
                            }
                            &after_time[1..]
                        } else {
                            after_time
                        };

                        if let Some(after_tz) = parse_timezone(tz_input, tz) {
                            if after_tz.is_empty() {
                                matches.push(StandardFormat::DateTime {
                                    date: date_fmt,
                                    time: TimeComponent {
                                        separator: sep,
                                        format: time_fmt,
                                        timezone: Some(tz),
                                        spaced_tz: spaced,
                                    },
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    /// Polars-compatible format string, built compositionally from components.
    pub fn polars_format(&self) -> String {
        match self {
            StandardFormat::DateOnly { date } => date.polars_date().to_string(),
            StandardFormat::DateTime { date, time } => {
                let tz_str = match time.timezone {
                    Some(tz) if time.spaced_tz => {
                        let mut s = String::from(" ");
                        s.push_str(tz.polars_tz());
                        s
                    }
                    Some(tz) => tz.polars_tz().to_string(),
                    None => String::new(),
                };
                format!(
                    "{}{}{}{}",
                    date.polars_date(),
                    time.separator.polars_sep(),
                    time.format.polars_time(),
                    tz_str,
                )
            }
        }
    }

    /// Validate a value against this specific format using compositional parsing.
    ///
    /// Unlike `parse()` which discovers all matching formats, this method checks
    /// only the specific components of `self`, making it more efficient for
    /// subsequent-value validation in the inference engine.
    pub(super) fn validates(&self, value: &str) -> bool {
        match self {
            StandardFormat::DateOnly { date } => {
                parse_date(value, *date).is_some_and(|r| r.is_empty())
            }
            StandardFormat::DateTime { date, time } => {
                let Some(after_date) = parse_date(value, *date) else {
                    return false;
                };
                let Some(after_sep) = parse_separator(after_date, time.separator) else {
                    return false;
                };
                let Some(after_time) = parse_time(after_sep, time.format) else {
                    return false;
                };
                match time.timezone {
                    None => after_time.is_empty(),
                    Some(tz) => {
                        let tz_input = if time.spaced_tz {
                            if after_time.is_empty() || after_time.as_bytes()[0] != b' ' {
                                return false;
                            }
                            &after_time[1..]
                        } else {
                            after_time
                        };
                        parse_timezone(tz_input, tz).is_some_and(|r| r.is_empty())
                    }
                }
            }
        }
    }
}
