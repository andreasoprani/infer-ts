/// Unix timestamp precision, distinguished by digit count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// Unix epoch timestamp (bare integer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnixFormat {
    pub precision: UnixPrecision,
}

impl UnixFormat {
    /// Polars-compatible format marker.
    pub fn polars_format(&self) -> &'static str {
        match self.precision {
            UnixPrecision::Seconds => "@unix_seconds",
            UnixPrecision::Milliseconds => "@unix_ms",
            UnixPrecision::Microseconds => "@unix_us",
            UnixPrecision::Nanoseconds => "@unix_ns",
        }
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
