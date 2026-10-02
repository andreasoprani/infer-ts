use super::date::DateFmt;
use super::time::*;

/// Time component of a datetime: separator + time format + optional timezone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TimeComponent {
    pub separator: Separator,
    pub format: TimeFmt,
    pub timezone: Option<Timezone>,
    /// Whether there is a space between time and timezone (e.g., `10:30:00 +05:30`).
    pub spaced_tz: bool,
}

/// A datetime format composed of date + time component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DateTimeFormat {
    pub date: DateFmt,
    pub time: TimeComponent,
}

impl DateTimeFormat {
    /// Parse a value and return all matching `DateTimeFormat`s.
    ///
    /// Compositional parsing: parse date → separator → time → timezone linearly,
    /// accepting all structurally valid combinations.
    pub fn parse(value: &str) -> Vec<DateTimeFormat> {
        let mut matches = Vec::new();

        for date_fmt in DateFmt::all() {
            let Some(remaining) = date_fmt.parse_date(value) else {
                continue;
            };

            if remaining.is_empty() {
                continue; // date-only → not our concern
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
        matches: &mut Vec<DateTimeFormat>,
    ) {
        for sep in [Separator::T, Separator::Space] {
            let Some(after_sep) = parse_separator(remaining, sep) else {
                continue;
            };

            for time_fmt in [
                TimeFmt::Hm,
                TimeFmt::Hms,
                TimeFmt::HmsFrac,
                TimeFmt::HmsCompact,
                TimeFmt::Hm12,
                TimeFmt::Hm12Compact,
                TimeFmt::Hms12,
                TimeFmt::Hms12Compact,
            ] {
                let Some(after_time) = parse_time(after_sep, time_fmt) else {
                    continue;
                };

                // No timezone
                if after_time.is_empty() {
                    matches.push(DateTimeFormat {
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
                for tz in [
                    Timezone::Utc,
                    Timezone::Offset,
                    Timezone::OffsetCompact,
                    Timezone::OffsetHour,
                ] {
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
                                matches.push(DateTimeFormat {
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
        let tz_str = match self.time.timezone {
            Some(tz) if self.time.spaced_tz => {
                let mut s = String::from(" ");
                s.push_str(tz.polars_tz());
                s
            }
            Some(tz) => tz.polars_tz().to_string(),
            None => String::new(),
        };
        format!(
            "{}{}{}{}",
            self.date.polars_date(),
            self.time.separator.polars_sep(),
            self.time.format.polars_time(),
            tz_str,
        )
    }

    /// Validate a value against this specific format using compositional parsing.
    pub(super) fn validates(&self, value: &str) -> bool {
        let Some(after_date) = self.date.parse_date(value) else {
            return false;
        };
        let Some(after_sep) = parse_separator(after_date, self.time.separator) else {
            return false;
        };
        let Some(after_time) = parse_time(after_sep, self.time.format) else {
            return false;
        };
        match self.time.timezone {
            None => after_time.is_empty(),
            Some(tz) => {
                let tz_input = if self.time.spaced_tz {
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
