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

import polars as pl
import pytest

import infer_ts


# ─── ISO 8601 ─────────────────────────────────────────────────────────────────


class TestISO8601:
    """ISO 8601 datetime formats with T separator."""

    def test_iso8601_plain(self):
        s = pl.Series("ts", ["2024-01-15T10:30:00", "2024-06-20T08:00:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].year == 2024
        assert result[0].month == 1
        assert result[0].day == 15

    def test_iso8601_utc(self):
        s = pl.Series("ts", ["2024-01-15T10:30:00Z", "2024-06-20T08:00:00Z"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[1].month == 6

    def test_iso8601_with_offset(self):
        s = pl.Series(
            "ts", ["2024-01-15T10:30:00+05:30", "2024-06-20T08:00:00-08:00"]
        )
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime

    def test_iso8601_fractional(self):
        s = pl.Series(
            "ts", ["2024-01-15T10:30:00.123456", "2024-06-20T08:00:00.999"]
        )
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime

    def test_iso8601_fractional_utc(self):
        s = pl.Series(
            "ts", ["2024-01-15T10:30:00.123Z", "2024-06-20T08:00:00.456789Z"]
        )
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime

    def test_iso8601_fractional_offset(self):
        s = pl.Series(
            "ts",
            [
                "2024-01-15T10:30:00.123+05:30",
                "2024-06-20T08:00:00.456789-08:00",
            ],
        )
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime

    def test_iso8601_with_compact_offset(self):
        """Compact offset without colon (e.g., +0530 instead of +05:30)."""
        s = pl.Series(
            "ts", ["2024-01-15T10:30:00+0530", "2024-06-20T08:00:00-0800"]
        )
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime

    def test_iso8601_fractional_compact_offset(self):
        """Fractional seconds with compact offset."""
        s = pl.Series(
            "ts",
            [
                "2024-01-15T10:30:00.123+0530",
                "2024-06-20T08:00:00.456789-0800",
            ],
        )
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime


# ─── Space-separated ──────────────────────────────────────────────────────────


class TestSpaceSeparated:
    """Space-separated datetime formats."""

    def test_space_datetime(self):
        s = pl.Series("ts", ["2024-01-15 10:30:00", "2024-06-20 08:00:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].hour == 10
        assert result[0].minute == 30

    def test_space_datetime_fractional(self):
        s = pl.Series(
            "ts", ["2024-01-15 10:30:00.123456", "2024-06-20 08:00:00.789"]
        )
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime

    def test_space_utc(self):
        s = pl.Series("ts", ["2024-01-15 10:30:00Z", "2024-06-20 08:00:00Z"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].hour == 10

    def test_space_offset(self):
        s = pl.Series(
            "ts", ["2024-01-15 10:30:00+05:30", "2024-06-20 08:00:00-08:00"]
        )
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime

    def test_space_offset_compact(self):
        s = pl.Series(
            "ts", ["2024-01-15 10:30:00+0530", "2024-06-20 08:00:00-0800"]
        )
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime

    def test_space_fractional_utc(self):
        s = pl.Series(
            "ts", ["2024-01-15 10:30:00.123Z", "2024-06-20 08:00:00.456789Z"]
        )
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime

    def test_space_fractional_offset(self):
        s = pl.Series(
            "ts",
            [
                "2024-01-15 10:30:00.123+05:30",
                "2024-06-20 08:00:00.456789-08:00",
            ],
        )
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime

    def test_space_fractional_offset_compact(self):
        s = pl.Series(
            "ts",
            [
                "2024-01-15 10:30:00.123+0530",
                "2024-06-20 08:00:00.456789-0800",
            ],
        )
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime


# ─── Slash dates (US/EU) ──────────────────────────────────────────────────────


class TestSlashDates:
    """US (mm/dd/yyyy) and EU (dd/mm/yyyy) slash date formats."""

    def test_us_slash_date(self):
        # day > 12 ensures unambiguous US format
        s = pl.Series("ts", ["01/15/2024", "06/20/2024"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Date or result.dtype == pl.Datetime
        # Verify it parsed as mm/dd (US): 01/15 = Jan 15
        assert result[0].month == 1
        assert result[0].day == 15

    def test_eu_slash_date(self):
        # first component > 12 ensures unambiguous EU format
        s = pl.Series("ts", ["15/01/2024", "20/06/2024"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Date or result.dtype == pl.Datetime
        # Verify it parsed as dd/mm (EU): 15/01 = Jan 15
        assert result[0].month == 1
        assert result[0].day == 15

    def test_us_slash_datetime(self):
        s = pl.Series("ts", ["01/15/2024 10:30:00", "06/20/2024 08:00:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].hour == 10

    def test_eu_slash_datetime(self):
        s = pl.Series("ts", ["15/01/2024 10:30:00", "20/06/2024 08:00:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].hour == 10

    def test_us_slash_date_short_year(self):
        s = pl.Series("ts", ["01/15/24", "06/20/24"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Date or result.dtype == pl.Datetime
        assert result[0].month == 1
        assert result[0].day == 15

    def test_eu_slash_date_short_year(self):
        s = pl.Series("ts", ["15/01/24", "20/06/24"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Date or result.dtype == pl.Datetime
        assert result[0].month == 1
        assert result[0].day == 15

    def test_us_slash_datetime_short_year(self):
        s = pl.Series("ts", ["01/15/24 10:30:00", "06/20/24 08:00:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].hour == 10

    def test_eu_slash_datetime_short_year(self):
        s = pl.Series("ts", ["15/01/24 10:30:00", "20/06/24 08:00:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].hour == 10


# ─── Compact ──────────────────────────────────────────────────────────────────


class TestCompact:
    """Compact date/datetime formats (no separators)."""

    def test_compact_date(self):
        s = pl.Series("ts", ["20240115", "20240620"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Date or result.dtype == pl.Datetime
        assert result[0].year == 2024
        assert result[0].month == 1
        assert result[0].day == 15

    def test_compact_datetime(self):
        s = pl.Series("ts", ["20240115T103000", "20240620T080000"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime


# ─── Unix epoch ───────────────────────────────────────────────────────────────


class TestUnixEpoch:
    """Unix epoch formats (integer timestamps)."""

    def test_unix_seconds(self):
        # 1705312200 = 2024-01-15 10:30:00 UTC
        s = pl.Series("ts", ["1705312200", "1705398600"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].year == 2024

    def test_unix_milliseconds(self):
        # 1705312200000 = 2024-01-15 10:30:00 UTC (ms)
        s = pl.Series("ts", ["1705312200000", "1705398600000"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].year == 2024

    def test_unix_microseconds(self):
        # 1705312200000000 = 2024-01-15 10:30:00 UTC (µs)
        s = pl.Series("ts", ["1705312200000000", "1705398600000000"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].year == 2024

    def test_unix_nanoseconds(self):
        # 1705312200000000000 = 2024-01-15 10:30:00 UTC (ns)
        s = pl.Series("ts", ["1705312200000000000", "1705398600000000000"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].year == 2024


# ─── 12-hour AM/PM ────────────────────────────────────────────────────────────


class TestAMPM:
    """12-hour AM/PM time formats."""

    def test_us_slash_ampm(self):
        s = pl.Series("ts", ["01/15/2024 2:30:00 PM", "06/20/2024 8:00:00 AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].hour == 14  # 2:30 PM = 14:30
        assert result[0].minute == 30

    def test_eu_slash_ampm(self):
        s = pl.Series("ts", ["15/01/2024 2:30:00 PM", "20/06/2024 8:00:00 AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].month == 1
        assert result[0].day == 15

    def test_iso_ampm(self):
        s = pl.Series("ts", ["2024-01-15 2:30:00 PM", "2024-06-20 8:00:00 AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].hour == 14

    def test_ampm_padded_hour(self):
        s = pl.Series("ts", ["01/15/2024 02:30:00 PM", "06/20/2024 08:00:00 AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].hour == 14

    def test_ampm_midnight_noon(self):
        s = pl.Series("ts", ["01/15/2024 12:00:00 AM", "01/15/2024 12:00:00 PM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].hour == 0   # 12:00 AM = midnight
        assert result[1].hour == 12  # 12:00 PM = noon

    def test_ampm_case_insensitive(self):
        s = pl.Series("ts", ["01/15/2024 2:30:00 pm", "01/15/2024 3:00:00 AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].hour == 14
        assert result[1].hour == 3

    def test_ampm_compact_us_slash(self):
        """AM/PM without space before suffix (e.g. 2:30:00PM)."""
        s = pl.Series("ts", ["01/15/2024 2:30:00PM", "06/20/2024 8:00:00AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].hour == 14
        assert result[0].minute == 30

    def test_ampm_compact_iso(self):
        s = pl.Series("ts", ["2024-01-15 2:30:00PM", "2024-06-20 8:00:00AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].hour == 14

    def test_us_slash_ampm_short_year(self):
        s = pl.Series("ts", ["01/15/24 2:30:00 PM", "06/20/24 8:00:00 AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].hour == 14
        assert result[0].minute == 30

    def test_ampm_compact_short_year(self):
        s = pl.Series("ts", ["01/15/24 2:30:00PM", "06/20/24 8:00:00AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].hour == 14
        assert result[0].minute == 30

    def test_ampm_compact_midnight_noon(self):
        s = pl.Series("ts", ["01/15/2024 12:00:00AM", "01/15/2024 12:00:00PM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].hour == 0   # 12:00AM = midnight
        assert result[1].hour == 12  # 12:00PM = noon


# ─── Dot-separated European dates ─────────────────────────────────────────────


class TestDotDates:
    """Dot-separated European date formats (DD.MM.YYYY, DD.MM.YY)."""

    def test_dot_eu_date(self):
        s = pl.Series("ts", ["15.01.2024", "20.06.2024"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Date or result.dtype == pl.Datetime
        assert result[0].month == 1
        assert result[0].day == 15

    def test_dot_eu_datetime(self):
        s = pl.Series("ts", ["15.01.2024 10:30:00", "20.06.2024 08:00:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].hour == 10

    def test_dot_eu_date_short_year(self):
        s = pl.Series("ts", ["15.01.24", "20.06.24"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Date or result.dtype == pl.Datetime
        assert result[0].month == 1
        assert result[0].day == 15

    def test_dot_eu_datetime_short_year(self):
        s = pl.Series("ts", ["15.01.24 10:30:00", "20.06.24 08:00:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].hour == 10

    def test_dot_eu_ampm(self):
        s = pl.Series("ts", ["15.01.2024 2:30:00 PM", "20.06.2024 8:00:00 AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].hour == 14
        assert result[0].minute == 30

    def test_dot_eu_ampm_compact(self):
        s = pl.Series("ts", ["15.01.2024 2:30:00PM", "20.06.2024 8:00:00AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].hour == 14
        assert result[0].minute == 30

    def test_dot_eu_format_assertions(self):
        """Verify the inferred format strings for dot-separated dates."""
        fmts = infer_ts.infer_format(["15.01.2024"])
        assert fmts == ["%d.%m.%Y"]

        fmts = infer_ts.infer_format(["15.01.24"])
        assert fmts == ["%d.%m.%y"]

        fmts = infer_ts.infer_format(["15.01.2024 10:30:00"])
        assert fmts == ["%d.%m.%Y %H:%M:%S"]

        fmts = infer_ts.infer_format(["15.01.24 10:30:00"])
        assert fmts == ["%d.%m.%y %H:%M:%S"]

        fmts = infer_ts.infer_format(["15.01.2024 2:30:00 PM"])
        assert fmts == ["%d.%m.%Y %I:%M:%S %p"]

        fmts = infer_ts.infer_format(["15.01.2024 2:30:00PM"])
        assert fmts == ["%d.%m.%Y %I:%M:%S%p"]


# ─── Month-name dates ────────────────────────────────────────────────────────


class TestMonthNameDates:
    """Month-name date formats (Jan 15, 2024 / 15 Jan 2024)."""

    def test_month_us_date(self):
        s = pl.Series("ts", ["Jan 15, 2024", "Jun 20, 2024"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Date or result.dtype == pl.Datetime
        assert result[0].month == 1
        assert result[0].day == 15

    def test_month_eu_date(self):
        s = pl.Series("ts", ["15 Jan 2024", "20 Jun 2024"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Date or result.dtype == pl.Datetime
        assert result[0].month == 1
        assert result[0].day == 15

    def test_month_us_datetime(self):
        s = pl.Series("ts", ["Jan 15, 2024 10:30:00", "Jun 20, 2024 08:00:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].hour == 10
        assert result[0].minute == 30

    def test_month_eu_datetime(self):
        s = pl.Series("ts", ["15 Jan 2024 10:30:00", "20 Jun 2024 08:00:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].hour == 10

    def test_month_us_date_short_year(self):
        s = pl.Series("ts", ["Jan 15, 24", "Jun 20, 24"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Date or result.dtype == pl.Datetime
        assert result[0].month == 1
        assert result[0].day == 15

    def test_month_eu_date_short_year(self):
        s = pl.Series("ts", ["15 Jan 24", "20 Jun 24"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Date or result.dtype == pl.Datetime
        assert result[0].month == 1
        assert result[0].day == 15

    def test_month_us_datetime_short_year(self):
        s = pl.Series("ts", ["Jan 15, 24 10:30:00", "Jun 20, 24 08:00:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].hour == 10

    def test_month_eu_datetime_short_year(self):
        s = pl.Series("ts", ["15 Jan 24 10:30:00", "20 Jun 24 08:00:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].hour == 10

    def test_month_us_ampm(self):
        s = pl.Series("ts", ["Jan 15, 2024 2:30:00 PM", "Jun 20, 2024 8:00:00 AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].hour == 14
        assert result[0].minute == 30

    def test_month_us_ampm_compact(self):
        s = pl.Series("ts", ["Jan 15, 2024 2:30:00PM", "Jun 20, 2024 8:00:00AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0].hour == 14
        assert result[0].minute == 30

    def test_month_name_format_assertions(self):
        """Verify the inferred format strings for month-name dates."""
        fmts = infer_ts.infer_format(["Jan 15, 2024"])
        assert fmts == ["%b %d, %Y"]

        fmts = infer_ts.infer_format(["15 Jan 2024"])
        assert fmts == ["%d %b %Y"]

        fmts = infer_ts.infer_format(["Jan 15, 24"])
        assert fmts == ["%b %d, %y"]

        fmts = infer_ts.infer_format(["15 Jan 24"])
        assert fmts == ["%d %b %y"]

        fmts = infer_ts.infer_format(["Jan 15, 2024 10:30:00"])
        assert fmts == ["%b %d, %Y %H:%M:%S"]

        fmts = infer_ts.infer_format(["15 Jan 2024 10:30:00"])
        assert fmts == ["%d %b %Y %H:%M:%S"]

        fmts = infer_ts.infer_format(["Jan 15, 2024 2:30:00 PM"])
        assert fmts == ["%b %d, %Y %I:%M:%S %p"]

        fmts = infer_ts.infer_format(["Jan 15, 2024 2:30:00PM"])
        assert fmts == ["%b %d, %Y %I:%M:%S%p"]


# ─── Edge cases ───────────────────────────────────────────────────────────────


class TestEdgeCases:
    """Edge cases and special scenarios."""

    def test_with_nulls(self):
        """Nulls should be preserved through the round-trip."""
        s = pl.Series("ts", ["2024-01-15T10:30:00", None, "2024-06-20T08:00:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert result[0] is not None
        assert result[1] is None
        assert result[2] is not None

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

        # Space-separated with timezone
        fmts = infer_ts.infer_format(["2024-01-15 10:30:00Z"])
        assert fmts == ["%Y-%m-%d %H:%M:%SZ"]

        fmts = infer_ts.infer_format(["2024-01-15 10:30:00+05:30"])
        assert fmts == ["%Y-%m-%d %H:%M:%S%:z"]

        fmts = infer_ts.infer_format(["2024-01-15 10:30:00+0530"])
        assert fmts == ["%Y-%m-%d %H:%M:%S%z"]

        fmts = infer_ts.infer_format(["2024-01-15 10:30:00.123Z"])
        assert fmts == ["%Y-%m-%d %H:%M:%S%.fZ"]

        fmts = infer_ts.infer_format(["2024-01-15 10:30:00.123+05:30"])
        assert fmts == ["%Y-%m-%d %H:%M:%S%.f%:z"]

        fmts = infer_ts.infer_format(["2024-01-15 10:30:00.123+0530"])
        assert fmts == ["%Y-%m-%d %H:%M:%S%.f%z"]

        # Short (2-digit) year
        fmts = infer_ts.infer_format(["01/15/24"])
        assert fmts == ["%m/%d/%y"]

        # AM/PM formats
        fmts = infer_ts.infer_format(["01/15/2024 2:30:00 PM"])
        assert fmts == ["%m/%d/%Y %I:%M:%S %p"]

        fmts = infer_ts.infer_format(["2024-01-15 2:30:00 PM"])
        assert fmts == ["%Y-%m-%d %I:%M:%S %p"]

        # AM/PM compact (no space before AM/PM)
        fmts = infer_ts.infer_format(["01/15/2024 2:30:00PM"])
        assert fmts == ["%m/%d/%Y %I:%M:%S%p"]

        fmts = infer_ts.infer_format(["2024-01-15 2:30:00PM"])
        assert fmts == ["%Y-%m-%d %I:%M:%S%p"]


# ─── to_datetime API tests ───────────────────────────────────────────────────


class TestToDatetimeAPI:
    """Tests for the to_datetime() convenience wrapper API."""

    def test_no_match_raises(self):
        """ValueError when no format matches."""
        s = pl.Series("ts", ["not a timestamp"])
        with pytest.raises(ValueError, match="No timestamp format"):
            infer_ts.to_datetime(s)

    def test_ambiguous_raises(self):
        """ValueError on ambiguous formats with raise_on_multiple=True."""
        # 01/02/2024 is ambiguous: could be US (Jan 2) or EU (1 Feb)
        s = pl.Series("ts", ["01/02/2024", "03/04/2024"])
        with pytest.raises(ValueError, match="Multiple timestamp formats"):
            infer_ts.to_datetime(s)

    def test_ambiguous_first_match(self):
        """Use first match when raise_on_multiple=False."""
        s = pl.Series("ts", ["01/02/2024", "03/04/2024"])
        result = infer_ts.to_datetime(s, raise_on_multiple=False)
        assert result.dtype == pl.Date or result.dtype == pl.Datetime

    def test_explicit_format_string(self):
        """Explicit format= bypasses inference."""
        s = pl.Series("ts", ["2024-01-15T10:30:00", "2024-06-20T08:00:00"])
        result = infer_ts.to_datetime(s, format="%Y-%m-%dT%H:%M:%S")

        assert result.dtype == pl.Datetime
        assert result[0].year == 2024

    def test_explicit_epoch_format(self):
        """Explicit format= with @unix_* marker."""
        s = pl.Series("ts", ["1705312200", "1705398600"])
        result = infer_ts.to_datetime(s, format="@unix_seconds")

        assert result.dtype == pl.Datetime
        assert result[0].year == 2024


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
