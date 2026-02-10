use super::date::*;

/// Time component of a standard datetime: separator + time format + optional timezone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimeComponent {
    pub separator: Separator,
    pub format: TimeFmt,
    pub timezone: Option<Timezone>,
}

/// A standard datetime format composed of date + optional time component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
                // Date-only (RFC 2822 always requires time+tz)
                if *date_fmt != DateFmt::Rfc2822 {
                    matches.push(StandardFormat::DateOnly { date: *date_fmt });
                }
                continue;
            }

            let spaced_tz = *date_fmt == DateFmt::Rfc2822;
            Self::parse_time_components(remaining, *date_fmt, spaced_tz, &mut matches);
        }

        matches
    }

    /// Try all separator × time × timezone combinations on the remaining string
    /// after the date has been consumed.
    fn parse_time_components(
        remaining: &str,
        date_fmt: DateFmt,
        spaced_tz: bool,
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
                        },
                    });
                    continue;
                }

                // Try each timezone variant
                for tz in [Timezone::Utc, Timezone::Offset, Timezone::OffsetCompact] {
                    // RFC 2822 has a space before the timezone offset
                    let tz_input = if spaced_tz {
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
                                },
                            });
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
            StandardFormat::DateTime { date, time } => format!(
                "{}{}{}{}",
                date.polars_date(),
                time.separator.polars_sep(),
                time.format.polars_time(),
                match time.timezone {
                    Some(tz) => tz.polars_tz(),
                    None => "",
                }
            ),
        }
    }

    /// Validate a value against this standard format using compositional parsing.
    pub(super) fn validates(&self, value: &str) -> bool {
        Self::parse(value).contains(self)
    }
}
