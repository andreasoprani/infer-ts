/// Unix timestamp precision, distinguished by digit count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

impl UnixPrecision {
    /// All precisions, in definition order.
    pub(super) const fn all() -> &'static [UnixPrecision] {
        &[
            UnixPrecision::Seconds,
            UnixPrecision::Milliseconds,
            UnixPrecision::Microseconds,
            UnixPrecision::Nanoseconds,
        ]
    }

}

#[cfg(test)]
impl UnixPrecision {
    /// Human-readable name for documentation tables.
    pub(super) fn name(&self) -> &'static str {
        match self {
            UnixPrecision::Seconds => "Seconds",
            UnixPrecision::Milliseconds => "Milliseconds",
            UnixPrecision::Microseconds => "Microseconds",
            UnixPrecision::Nanoseconds => "Nanoseconds",
        }
    }

    /// Canonical example value for documentation tables.
    pub(super) fn example(&self) -> &'static str {
        match self {
            UnixPrecision::Seconds => "1705312200",
            UnixPrecision::Milliseconds => "1705312200000",
            UnixPrecision::Microseconds => "1705312200000000",
            UnixPrecision::Nanoseconds => "1705312200000000000",
        }
    }

    /// Digit-count range string for documentation tables.
    pub(super) fn digits(&self) -> &'static str {
        match self {
            UnixPrecision::Seconds => "9\u{2013}10",
            UnixPrecision::Milliseconds => "11\u{2013}13",
            UnixPrecision::Microseconds => "14\u{2013}16",
            UnixPrecision::Nanoseconds => "17\u{2013}19",
        }
    }

    /// Approximate date range string for documentation tables.
    pub(super) fn date_range(&self) -> &'static str {
        match self {
            UnixPrecision::Seconds => "1973-03-03 \u{2026} 2286-11-20",
            UnixPrecision::Milliseconds => "1970 \u{2026} 2286 (ms)",
            UnixPrecision::Microseconds => "1970 \u{2026} 2286 (\u{b5}s)",
            UnixPrecision::Nanoseconds => "1677 \u{2026} 2262 (ns, i64)",
        }
    }
}

/// Unix epoch timestamp (bare integer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UnixFormat {
    pub precision: UnixPrecision,
}

impl UnixFormat {
    /// Parse a value and return all matching `UnixFormat`s.
    pub fn parse(value: &str) -> Vec<UnixFormat> {
        UnixPrecision::all()
            .iter()
            .filter_map(|&precision| {
                let uf = UnixFormat { precision };
                if uf.validates(value) {
                    Some(uf)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Polars-compatible format marker.
    pub fn polars_format(&self) -> String {
        match self.precision {
            UnixPrecision::Seconds => "@unix_seconds",
            UnixPrecision::Milliseconds => "@unix_ms",
            UnixPrecision::Microseconds => "@unix_us",
            UnixPrecision::Nanoseconds => "@unix_ns",
        }
        .to_string()
    }

    /// Validate a value as a Unix timestamp of this precision.
    pub(super) fn validates(&self, value: &str) -> bool {
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
