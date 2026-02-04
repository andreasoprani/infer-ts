"""Integration tests: round-trip with Polars.

These tests verify that infer_format returns format strings that Polars can
actually use to parse the original values back to Datetime dtype.

Coverage (per TODO v0.1):
- ISO 8601 datetime
- Space-separated datetime
- US/EU slash dates
- Compact date
- Unix epoch (seconds, ms, µs, ns)
"""

from typing import Literal

import polars as pl
import pytest

import infer_ts

# ─── Epoch unit mapping ───────────────────────────────────────────────────────
# infer_ts returns @-prefixed markers for epoch columns. These require integer
# casting rather than format-string parsing.

EPOCH_UNITS: dict[str, tuple[Literal["ns", "us", "ms"], int]] = {
    "@unix_seconds": ("ms", 1000),  # (target unit, multiplier from source)
    "@unix_ms": ("ms", 1),
    "@unix_us": ("us", 1),
    "@unix_ns": ("ns", 1),
}


def parse_with_inferred_format(
    df: pl.DataFrame, col: str
) -> tuple[pl.DataFrame, list[str]]:
    """Apply inferred format to parse a string column to Datetime.

    For string-based formats: uses str.to_datetime with the format string.
    For epoch formats: casts to Int64 then to Datetime with appropriate unit.

    Returns:
        A tuple of (parsed DataFrame, list of inferred format strings).
    """
    values = df[col].to_list()
    fmts = infer_ts.infer_format(values)

    assert fmts, f"infer_format returned empty list for column {col}"
    fmt = fmts[0]  # Use first format for parsing

    if fmt in EPOCH_UNITS:
        unit, multiplier = EPOCH_UNITS[fmt]
        result = df.with_columns(
            (pl.col(col).cast(pl.Int64) * multiplier).cast(pl.Datetime(unit))
        )
    else:
        result = df.with_columns(pl.col(col).str.to_datetime(format=fmt))

    return result, fmts


# ─── ISO 8601 ─────────────────────────────────────────────────────────────────


class TestISO8601:
    """ISO 8601 datetime formats with T separator."""

    def test_iso8601_plain(self):
        df = pl.DataFrame({"ts": ["2024-01-15T10:30:00", "2024-06-20T08:00:00"]})
        result, fmts = parse_with_inferred_format(df, "ts")

        assert result["ts"].dtype == pl.Datetime
        assert result["ts"][0].year == 2024
        assert result["ts"][0].month == 1
        assert result["ts"][0].day == 15
        assert fmts == ["%Y-%m-%dT%H:%M:%S"]

    def test_iso8601_utc(self):
        df = pl.DataFrame({"ts": ["2024-01-15T10:30:00Z", "2024-06-20T08:00:00Z"]})
        result, fmts = parse_with_inferred_format(df, "ts")

        assert result["ts"].dtype == pl.Datetime
        assert result["ts"][1].month == 6
        assert fmts == ["%Y-%m-%dT%H:%M:%SZ"]

    def test_iso8601_with_offset(self):
        df = pl.DataFrame(
            {"ts": ["2024-01-15T10:30:00+05:30", "2024-06-20T08:00:00-08:00"]}
        )
        result, fmts = parse_with_inferred_format(df, "ts")

        assert result["ts"].dtype == pl.Datetime
        assert fmts == ["%Y-%m-%dT%H:%M:%S%:z"]

    def test_iso8601_fractional(self):
        df = pl.DataFrame(
            {"ts": ["2024-01-15T10:30:00.123456", "2024-06-20T08:00:00.999"]}
        )
        result, fmts = parse_with_inferred_format(df, "ts")

        assert result["ts"].dtype == pl.Datetime
        assert fmts == ["%Y-%m-%dT%H:%M:%S%.f"]

    def test_iso8601_fractional_utc(self):
        df = pl.DataFrame(
            {"ts": ["2024-01-15T10:30:00.123Z", "2024-06-20T08:00:00.456789Z"]}
        )
        result, fmts = parse_with_inferred_format(df, "ts")

        assert result["ts"].dtype == pl.Datetime
        assert fmts == ["%Y-%m-%dT%H:%M:%S%.fZ"]

    def test_iso8601_fractional_offset(self):
        df = pl.DataFrame(
            {
                "ts": [
                    "2024-01-15T10:30:00.123+05:30",
                    "2024-06-20T08:00:00.456789-08:00",
                ]
            }
        )
        result, fmts = parse_with_inferred_format(df, "ts")

        assert result["ts"].dtype == pl.Datetime
        assert fmts == ["%Y-%m-%dT%H:%M:%S%.f%:z"]

    def test_iso8601_with_compact_offset(self):
        """Compact offset without colon (e.g., +0530 instead of +05:30)."""
        df = pl.DataFrame(
            {"ts": ["2024-01-15T10:30:00+0530", "2024-06-20T08:00:00-0800"]}
        )
        result, fmts = parse_with_inferred_format(df, "ts")

        assert result["ts"].dtype == pl.Datetime
        assert fmts == ["%Y-%m-%dT%H:%M:%S%z"]

    def test_iso8601_fractional_compact_offset(self):
        """Fractional seconds with compact offset."""
        df = pl.DataFrame(
            {
                "ts": [
                    "2024-01-15T10:30:00.123+0530",
                    "2024-06-20T08:00:00.456789-0800",
                ]
            }
        )
        result, fmts = parse_with_inferred_format(df, "ts")

        assert result["ts"].dtype == pl.Datetime
        assert fmts == ["%Y-%m-%dT%H:%M:%S%.f%z"]


# ─── Space-separated ──────────────────────────────────────────────────────────


class TestSpaceSeparated:
    """Space-separated datetime formats."""

    def test_space_datetime(self):
        df = pl.DataFrame({"ts": ["2024-01-15 10:30:00", "2024-06-20 08:00:00"]})
        result, fmts = parse_with_inferred_format(df, "ts")

        assert result["ts"].dtype == pl.Datetime
        assert result["ts"][0].hour == 10
        assert result["ts"][0].minute == 30
        assert fmts == ["%Y-%m-%d %H:%M:%S"]

    def test_space_datetime_fractional(self):
        df = pl.DataFrame(
            {"ts": ["2024-01-15 10:30:00.123456", "2024-06-20 08:00:00.789"]}
        )
        result, fmts = parse_with_inferred_format(df, "ts")

        assert result["ts"].dtype == pl.Datetime
        assert fmts == ["%Y-%m-%d %H:%M:%S%.f"]


# ─── Slash dates (US/EU) ──────────────────────────────────────────────────────


class TestSlashDates:
    """US (mm/dd/yyyy) and EU (dd/mm/yyyy) slash date formats."""

    def test_us_slash_date(self):
        # day > 12 ensures unambiguous US format
        df = pl.DataFrame({"ts": ["01/15/2024", "06/20/2024"]})
        result, fmts = parse_with_inferred_format(df, "ts")

        assert result["ts"].dtype == pl.Date or result["ts"].dtype == pl.Datetime
        # Verify it parsed as mm/dd (US): 01/15 = Jan 15
        assert result["ts"][0].month == 1
        assert result["ts"][0].day == 15
        assert fmts == ["%m/%d/%Y"]

    def test_eu_slash_date(self):
        # first component > 12 ensures unambiguous EU format
        df = pl.DataFrame({"ts": ["15/01/2024", "20/06/2024"]})
        result, fmts = parse_with_inferred_format(df, "ts")

        assert result["ts"].dtype == pl.Date or result["ts"].dtype == pl.Datetime
        # Verify it parsed as dd/mm (EU): 15/01 = Jan 15
        assert result["ts"][0].month == 1
        assert result["ts"][0].day == 15
        assert fmts == ["%d/%m/%Y"]

    def test_us_slash_datetime(self):
        df = pl.DataFrame({"ts": ["01/15/2024 10:30:00", "06/20/2024 08:00:00"]})
        result, fmts = parse_with_inferred_format(df, "ts")

        assert result["ts"].dtype == pl.Datetime
        assert result["ts"][0].hour == 10
        assert fmts == ["%m/%d/%Y %H:%M:%S"]

    def test_eu_slash_datetime(self):
        df = pl.DataFrame({"ts": ["15/01/2024 10:30:00", "20/06/2024 08:00:00"]})
        result, fmts = parse_with_inferred_format(df, "ts")

        assert result["ts"].dtype == pl.Datetime
        assert result["ts"][0].hour == 10
        assert fmts == ["%d/%m/%Y %H:%M:%S"]


# ─── Compact ──────────────────────────────────────────────────────────────────


class TestCompact:
    """Compact date/datetime formats (no separators)."""

    def test_compact_date(self):
        df = pl.DataFrame({"ts": ["20240115", "20240620"]})
        result, fmts = parse_with_inferred_format(df, "ts")

        assert result["ts"].dtype == pl.Date or result["ts"].dtype == pl.Datetime
        assert result["ts"][0].year == 2024
        assert result["ts"][0].month == 1
        assert result["ts"][0].day == 15
        assert fmts == ["%Y%m%d"]

    def test_compact_datetime(self):
        df = pl.DataFrame({"ts": ["20240115T103000", "20240620T080000"]})
        result, fmts = parse_with_inferred_format(df, "ts")

        assert result["ts"].dtype == pl.Datetime
        assert fmts == ["%Y%m%dT%H%M%S"]


# ─── Unix epoch ───────────────────────────────────────────────────────────────


class TestUnixEpoch:
    """Unix epoch formats (integer timestamps)."""

    def test_unix_seconds(self):
        # 1705312200 = 2024-01-15 10:30:00 UTC
        df = pl.DataFrame({"ts": ["1705312200", "1705398600"]})
        result, fmts = parse_with_inferred_format(df, "ts")

        assert result["ts"].dtype == pl.Datetime
        # Verify approximate date (exact depends on timezone handling)
        assert result["ts"][0].year == 2024
        assert fmts == ["@unix_seconds"]

    def test_unix_milliseconds(self):
        # 1705312200000 = 2024-01-15 10:30:00 UTC (ms)
        df = pl.DataFrame({"ts": ["1705312200000", "1705398600000"]})
        result, fmts = parse_with_inferred_format(df, "ts")

        assert result["ts"].dtype == pl.Datetime
        assert result["ts"][0].year == 2024
        assert fmts == ["@unix_ms"]

    def test_unix_microseconds(self):
        # 1705312200000000 = 2024-01-15 10:30:00 UTC (µs)
        df = pl.DataFrame({"ts": ["1705312200000000", "1705398600000000"]})
        result, fmts = parse_with_inferred_format(df, "ts")

        assert result["ts"].dtype == pl.Datetime
        assert result["ts"][0].year == 2024
        assert fmts == ["@unix_us"]

    def test_unix_nanoseconds(self):
        # 1705312200000000000 = 2024-01-15 10:30:00 UTC (ns)
        df = pl.DataFrame({"ts": ["1705312200000000000", "1705398600000000000"]})
        result, fmts = parse_with_inferred_format(df, "ts")

        assert result["ts"].dtype == pl.Datetime
        assert result["ts"][0].year == 2024
        assert fmts == ["@unix_ns"]


# ─── Edge cases ───────────────────────────────────────────────────────────────


class TestEdgeCases:
    """Edge cases and special scenarios."""

    def test_with_nulls(self):
        """Nulls should be preserved through the round-trip."""
        df = pl.DataFrame({"ts": ["2024-01-15T10:30:00", None, "2024-06-20T08:00:00"]})
        result, fmts = parse_with_inferred_format(df, "ts")

        assert result["ts"].dtype == pl.Datetime
        assert result["ts"][0] is not None
        assert result["ts"][1] is None
        assert result["ts"][2] is not None
        assert fmts == ["%Y-%m-%dT%H:%M:%S"]

    def test_inferred_format_matches_expectation(self):
        """Verify the inferred format string is what we expect."""
        fmts = infer_ts.infer_format(["2024-01-15T10:30:00"])
        assert fmts == ["%Y-%m-%dT%H:%M:%S"]

        fmts = infer_ts.infer_format(["2024-01-15 10:30:00"])
        assert fmts == ["%Y-%m-%d %H:%M:%S"]

        fmts = infer_ts.infer_format(["1705312200"])
        assert fmts == ["@unix_seconds"]

        # Verify offset formats distinguish colon vs compact
        fmts = infer_ts.infer_format(["2024-01-15T10:30:00+05:30"])
        assert fmts == ["%Y-%m-%dT%H:%M:%S%:z"]

        fmts = infer_ts.infer_format(["2024-01-15T10:30:00+0530"])
        assert fmts == ["%Y-%m-%dT%H:%M:%S%z"]


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
