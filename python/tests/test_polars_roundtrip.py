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

from .conftest import _d, _dt

# ─── ISO 8601 ─────────────────────────────────────────────────────────────────


class TestISO8601:
    """ISO 8601 datetime formats with T separator."""

    def test_iso8601_plain(self):
        s = pl.Series("ts", ["2024-01-15T10:30:00", "2024-06-20T08:00:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        v = _dt(result, 0)
        assert v.year == 2024
        assert v.month == 1
        assert v.day == 15

    def test_iso8601_without_seconds(self):
        s = pl.Series("ts", ["2024-01-15T10:30", "2024-06-20T08:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        v = _dt(result, 0)
        assert v.hour == 10
        assert v.minute == 30
        assert v.second == 0

    def test_iso8601_unpadded_date_without_seconds(self):
        s = pl.Series("ts", ["2024-1-15T10:30", "2024-6-20T08:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert _dt(result, 0).month == 1

    def test_iso8601_utc(self):
        s = pl.Series("ts", ["2024-01-15T10:30:00Z", "2024-06-20T08:00:00Z"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert _dt(result, 1).month == 6

    def test_iso8601_with_offset(self):
        s = pl.Series("ts", ["2024-01-15T10:30:00+05:30", "2024-06-20T08:00:00-08:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime

    def test_iso8601_fractional(self):
        s = pl.Series("ts", ["2024-01-15T10:30:00.123456", "2024-06-20T08:00:00.999"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime

    def test_iso8601_fractional_utc(self):
        s = pl.Series("ts", ["2024-01-15T10:30:00.123Z", "2024-06-20T08:00:00.456789Z"])
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
        s = pl.Series("ts", ["2024-01-15T10:30:00+0530", "2024-06-20T08:00:00-0800"])
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
        v = _dt(result, 0)
        assert v.hour == 10
        assert v.minute == 30

    def test_space_datetime_fractional(self):
        s = pl.Series("ts", ["2024-01-15 10:30:00.123456", "2024-06-20 08:00:00.789"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime

    def test_space_utc(self):
        s = pl.Series("ts", ["2024-01-15 10:30:00Z", "2024-06-20 08:00:00Z"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert _dt(result, 0).hour == 10

    def test_space_offset(self):
        s = pl.Series("ts", ["2024-01-15 10:30:00+05:30", "2024-06-20 08:00:00-08:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime

    def test_space_offset_compact(self):
        s = pl.Series("ts", ["2024-01-15 10:30:00+0530", "2024-06-20 08:00:00-0800"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime

    def test_space_fractional_utc(self):
        s = pl.Series("ts", ["2024-01-15 10:30:00.123Z", "2024-06-20 08:00:00.456789Z"])
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

        assert result.dtype == pl.Datetime
        # Verify it parsed as mm/dd (US): 01/15 = Jan 15
        v = _d(result, 0)
        assert v.month == 1
        assert v.day == 15

    def test_us_slash_date_unpadded(self):
        # day > 12 ensures unambiguous US format; month may omit leading zero
        s = pl.Series("ts", ["1/15/2024", "6/20/2024"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        v = _d(result, 0)
        assert v.month == 1
        assert v.day == 15

    def test_eu_slash_date(self):
        # first component > 12 ensures unambiguous EU format
        s = pl.Series("ts", ["15/01/2024", "20/06/2024"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        # Verify it parsed as dd/mm (EU): 15/01 = Jan 15
        v = _d(result, 0)
        assert v.month == 1
        assert v.day == 15

    def test_eu_slash_date_unpadded(self):
        # first component > 12 ensures unambiguous EU format; month may omit leading zero
        s = pl.Series("ts", ["15/1/2024", "20/6/2024"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        v = _d(result, 0)
        assert v.month == 1
        assert v.day == 15

    def test_us_slash_datetime(self):
        s = pl.Series("ts", ["01/15/2024 10:30:00", "06/20/2024 08:00:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert _dt(result, 0).hour == 10

    def test_eu_slash_datetime(self):
        s = pl.Series("ts", ["15/01/2024 10:30:00", "20/06/2024 08:00:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert _dt(result, 0).hour == 10

    def test_us_slash_date_short_year(self):
        s = pl.Series("ts", ["01/15/24", "06/20/24"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        v = _d(result, 0)
        assert v.month == 1
        assert v.day == 15

    def test_eu_slash_date_short_year(self):
        s = pl.Series("ts", ["15/01/24", "20/06/24"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        v = _d(result, 0)
        assert v.month == 1
        assert v.day == 15

    def test_us_slash_datetime_short_year(self):
        s = pl.Series("ts", ["01/15/24 10:30:00", "06/20/24 08:00:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert _dt(result, 0).hour == 10

    def test_eu_slash_datetime_short_year(self):
        s = pl.Series("ts", ["15/01/24 10:30:00", "20/06/24 08:00:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert _dt(result, 0).hour == 10

    def test_slash_short_unpadded_ambiguous_infer_format(self):
        fmts = infer_ts.infer_format(["1/1/26"], exhaustive=True)
        assert fmts == ["%d/%m/%y", "%m/%d/%y"]

    def test_us_slash_short_datetime_unpadded_without_seconds(self):
        s = pl.Series("ts", ["1/15/26 10:30", "6/20/26 08:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert _dt(result, 0).hour == 10
        assert _dt(result, 0).second == 0


# ─── Compact ──────────────────────────────────────────────────────────────────


class TestCompact:
    """Compact date/datetime formats (no separators)."""

    def test_compact_date(self):
        s = pl.Series("ts", ["20240115", "20240620"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        v = _d(result, 0)
        assert v.year == 2024
        assert v.month == 1
        assert v.day == 15

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
        assert _dt(result, 0).year == 2024

    def test_unix_milliseconds(self):
        # 1705312200000 = 2024-01-15 10:30:00 UTC (ms)
        s = pl.Series("ts", ["1705312200000", "1705398600000"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert _dt(result, 0).year == 2024

    def test_unix_microseconds(self):
        # 1705312200000000 = 2024-01-15 10:30:00 UTC (µs)
        s = pl.Series("ts", ["1705312200000000", "1705398600000000"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert _dt(result, 0).year == 2024

    def test_unix_nanoseconds(self):
        # 1705312200000000000 = 2024-01-15 10:30:00 UTC (ns)
        s = pl.Series("ts", ["1705312200000000000", "1705398600000000000"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert _dt(result, 0).year == 2024


# ─── 12-hour AM/PM ────────────────────────────────────────────────────────────


class TestAMPM:
    """12-hour AM/PM time formats."""

    def test_us_slash_ampm(self):
        s = pl.Series("ts", ["01/15/2024 2:30:00 PM", "06/20/2024 8:00:00 AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        v = _dt(result, 0)
        assert v.hour == 14  # 2:30 PM = 14:30
        assert v.minute == 30

    def test_eu_slash_ampm(self):
        s = pl.Series("ts", ["15/01/2024 2:30:00 PM", "20/06/2024 8:00:00 AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        v = _dt(result, 0)
        assert v.month == 1
        assert v.day == 15

    def test_iso_ampm(self):
        s = pl.Series("ts", ["2024-01-15 2:30:00 PM", "2024-06-20 8:00:00 AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert _dt(result, 0).hour == 14

    def test_ampm_padded_hour(self):
        s = pl.Series("ts", ["01/15/2024 02:30:00 PM", "06/20/2024 08:00:00 AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert _dt(result, 0).hour == 14

    def test_ampm_midnight_noon(self):
        s = pl.Series("ts", ["01/15/2024 12:00:00 AM", "01/15/2024 12:00:00 PM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert _dt(result, 0).hour == 0  # 12:00 AM = midnight
        assert _dt(result, 1).hour == 12  # 12:00 PM = noon

    def test_ampm_case_insensitive(self):
        s = pl.Series("ts", ["01/15/2024 2:30:00 pm", "01/15/2024 3:00:00 AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert _dt(result, 0).hour == 14
        assert _dt(result, 1).hour == 3

    def test_ampm_compact_us_slash(self):
        """AM/PM without space before suffix (e.g. 2:30:00PM)."""
        s = pl.Series("ts", ["01/15/2024 2:30:00PM", "06/20/2024 8:00:00AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        v = _dt(result, 0)
        assert v.hour == 14
        assert v.minute == 30

    def test_ampm_compact_iso(self):
        s = pl.Series("ts", ["2024-01-15 2:30:00PM", "2024-06-20 8:00:00AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert _dt(result, 0).hour == 14

    def test_ampm_without_seconds(self):
        s = pl.Series("ts", ["01/15/2024 2:30 PM", "06/20/2024 8:00 AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        v = _dt(result, 0)
        assert v.hour == 14
        assert v.minute == 30
        assert v.second == 0

    def test_ampm_compact_without_seconds(self):
        s = pl.Series("ts", ["01/15/2024 2:30PM", "06/20/2024 8:00AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert _dt(result, 0).hour == 14

    def test_us_slash_ampm_short_year(self):
        s = pl.Series("ts", ["01/15/24 2:30:00 PM", "06/20/24 8:00:00 AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        v = _dt(result, 0)
        assert v.hour == 14
        assert v.minute == 30

    def test_ampm_compact_short_year(self):
        s = pl.Series("ts", ["01/15/24 2:30:00PM", "06/20/24 8:00:00AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        v = _dt(result, 0)
        assert v.hour == 14
        assert v.minute == 30

    def test_ampm_compact_midnight_noon(self):
        s = pl.Series("ts", ["01/15/2024 12:00:00AM", "01/15/2024 12:00:00PM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert _dt(result, 0).hour == 0  # 12:00AM = midnight
        assert _dt(result, 1).hour == 12  # 12:00PM = noon


# ─── Dot-separated European dates ─────────────────────────────────────────────


class TestDotDates:
    """Dot-separated European date formats (DD.MM.YYYY, DD.MM.YY)."""

    def test_dot_eu_date(self):
        s = pl.Series("ts", ["15.01.2024", "20.06.2024"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        v = _d(result, 0)
        assert v.month == 1
        assert v.day == 15

    def test_dot_eu_date_unpadded(self):
        s = pl.Series("ts", ["15.1.2024", "20.6.2024"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert _d(result, 0).month == 1

    def test_dot_eu_datetime(self):
        s = pl.Series("ts", ["15.01.2024 10:30:00", "20.06.2024 08:00:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert _dt(result, 0).hour == 10

    def test_dot_eu_date_short_year(self):
        s = pl.Series("ts", ["15.01.24", "20.06.24"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        v = _d(result, 0)
        assert v.month == 1
        assert v.day == 15

    def test_dot_eu_datetime_short_year(self):
        s = pl.Series("ts", ["15.01.24 10:30:00", "20.06.24 08:00:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert _dt(result, 0).hour == 10

    def test_dot_eu_ampm(self):
        s = pl.Series("ts", ["15.01.2024 2:30:00 PM", "20.06.2024 8:00:00 AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        v = _dt(result, 0)
        assert v.hour == 14
        assert v.minute == 30

    def test_dot_eu_ampm_compact(self):
        s = pl.Series("ts", ["15.01.2024 2:30:00PM", "20.06.2024 8:00:00AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        v = _dt(result, 0)
        assert v.hour == 14
        assert v.minute == 30

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

        assert result.dtype == pl.Datetime
        v = _d(result, 0)
        assert v.month == 1
        assert v.day == 15

    def test_month_eu_date(self):
        s = pl.Series("ts", ["15 Jan 2024", "20 Jun 2024"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        v = _d(result, 0)
        assert v.month == 1
        assert v.day == 15

    def test_month_us_datetime(self):
        s = pl.Series("ts", ["Jan 15, 2024 10:30:00", "Jun 20, 2024 08:00:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        v = _dt(result, 0)
        assert v.hour == 10
        assert v.minute == 30

    def test_month_eu_datetime(self):
        s = pl.Series("ts", ["15 Jan 2024 10:30:00", "20 Jun 2024 08:00:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert _dt(result, 0).hour == 10

    def test_month_us_date_short_year(self):
        s = pl.Series("ts", ["Jan 15, 24", "Jun 20, 24"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        v = _d(result, 0)
        assert v.month == 1
        assert v.day == 15

    def test_month_eu_date_short_year(self):
        s = pl.Series("ts", ["15 Jan 24", "20 Jun 24"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        v = _d(result, 0)
        assert v.month == 1
        assert v.day == 15

    def test_month_us_datetime_short_year(self):
        s = pl.Series("ts", ["Jan 15, 24 10:30:00", "Jun 20, 24 08:00:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert _dt(result, 0).hour == 10

    def test_month_eu_datetime_short_year(self):
        s = pl.Series("ts", ["15 Jan 24 10:30:00", "20 Jun 24 08:00:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        assert _dt(result, 0).hour == 10

    def test_month_us_ampm(self):
        s = pl.Series("ts", ["Jan 15, 2024 2:30:00 PM", "Jun 20, 2024 8:00:00 AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        v = _dt(result, 0)
        assert v.hour == 14
        assert v.minute == 30

    def test_month_us_ampm_compact(self):
        s = pl.Series("ts", ["Jan 15, 2024 2:30:00PM", "Jun 20, 2024 8:00:00AM"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        v = _dt(result, 0)
        assert v.hour == 14
        assert v.minute == 30

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


# ─── RFC 2822 ────────────────────────────────────────────────────────────────


class TestRfc2822:
    """RFC 2822 / email-style datetime formats."""

    def test_rfc2822_compact_offset(self):
        s = pl.Series("ts", ["Mon, 15 Jan 2024 10:30:00 +0530", "Thu, 20 Jun 2024 08:00:00 -0800"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        v = _dt(result, 0)
        assert v.month == 1
        assert v.day == 15

    def test_rfc2822_colon_offset(self):
        s = pl.Series(
            "ts",
            [
                "Mon, 15 Jan 2024 10:30:00 +05:30",
                "Thu, 20 Jun 2024 08:00:00 -08:00",
            ],
        )
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        v = _dt(result, 0)
        assert v.month == 1
        assert v.day == 15

    def test_rfc2822_format_assertions(self):
        """Verify the inferred format strings for RFC 2822 dates."""
        fmts = infer_ts.infer_format(["Mon, 15 Jan 2024 10:30:00 +0530"])
        assert fmts == ["%a, %d %b %Y %H:%M:%S %z"]

        fmts = infer_ts.infer_format(["Mon, 15 Jan 2024 10:30:00 +05:30"])
        assert fmts == ["%a, %d %b %Y %H:%M:%S %:z"]


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

    def test_all_nulls_returns_no_format(self):
        """All-null series returns empty format list (can't infer from no data)."""
        fmts = infer_ts.infer_format([None, None, None])
        assert fmts == []

    def test_all_nulls_to_datetime_returns_null_datetime(self):
        """All-null series casts to all-null Datetime series."""
        s = pl.Series("ts", [None, None, None], dtype=pl.Utf8)
        result = infer_ts.to_datetime(s)
        assert result.dtype == pl.Datetime
        assert result.null_count() == 3
        assert len(result) == 3

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
        """ComputeError when no format matches."""
        s = pl.Series("ts", ["not a timestamp"])
        with pytest.raises(pl.exceptions.ComputeError, match="no timestamp format"):
            _ = infer_ts.to_datetime(s)

    def test_ambiguous_raises(self):
        """ComputeError on ambiguous formats with raise_on_multiple=True (default)."""
        s = pl.Series("ts", ["01/02/2024", "03/04/2024"])
        with pytest.raises(pl.exceptions.ComputeError, match="multiple timestamp formats"):
            _ = infer_ts.to_datetime(s)

    def test_date_preference_eu(self):
        """date_preference='eu' selects EU interpretation when raise_on_multiple=False."""
        s = pl.Series("ts", ["01/02/2024", "03/04/2024"])
        result = infer_ts.to_datetime(s, raise_on_multiple=False, date_preference="eu")
        assert result.dtype == pl.Datetime
        assert _dt(result, 0).month == 2  # EU: 01/02 → Feb

    def test_date_preference_us(self):
        """date_preference='us' selects US interpretation when raise_on_multiple=False."""
        s = pl.Series("ts", ["01/02/2024", "03/04/2024"])
        result = infer_ts.to_datetime(s, raise_on_multiple=False, date_preference="us")
        assert result.dtype == pl.Datetime
        assert _dt(result, 0).month == 1  # US: 01/02 → Jan

    def test_ambiguous_first_match(self):
        """Use first match when raise_on_multiple=False."""
        s = pl.Series("ts", ["01/02/2024", "03/04/2024"])
        result = infer_ts.to_datetime(s, raise_on_multiple=False)
        assert result.dtype == pl.Datetime


# ─── Iterator / streaming interface ──────────────────────────────────────────


class TestIteratorInterface:
    """Tests for the streaming iterator path (infer_format with iterables)."""

    def test_generator(self):
        """infer_format accepts a generator (lazy, no list materialisation)."""

        def gen():
            yield "2024-01-15T10:30:00"
            yield "2024-06-20T08:00:00"

        fmts = infer_ts.infer_format(gen())
        assert fmts == ["%Y-%m-%dT%H:%M:%S"]

    def test_generator_with_nones(self):
        """Generator yielding None values are skipped."""

        def gen():
            yield None
            yield "2024-01-15T10:30:00"
            yield None

        fmts = infer_ts.infer_format(gen())
        assert fmts == ["%Y-%m-%dT%H:%M:%S"]

    def test_generator_early_exit(self):
        """Non-exhaustive mode stops pulling from the generator early."""
        pulled = []

        def gen():
            # First value: unambiguous format (only one candidate)
            pulled.append(1)
            yield "2024-01-15T10:30:00+05:30"
            # This value should NOT be pulled in non-exhaustive mode
            pulled.append(2)
            yield "GARBAGE"

        fmts = infer_ts.infer_format(gen())
        assert fmts == ["%Y-%m-%dT%H:%M:%S%:z"]
        assert pulled == [1], "should have stopped after first value (early exit)"

    def test_tuple_iterable(self):
        """Accepts any iterable, not just generators."""
        fmts = infer_ts.infer_format(iter(("2024-01-15", "2024-06-20")))
        assert fmts == ["%Y-%m-%d"]

    def test_empty_generator(self):
        """Empty generator returns empty list."""
        fmts = infer_ts.infer_format(iter([]))
        assert fmts == []

    def test_list_still_works(self):
        """List input still dispatches to the fast list path."""
        fmts = infer_ts.infer_format(["2024-01-15T10:30:00"])
        assert fmts == ["%Y-%m-%dT%H:%M:%S"]


# ─── exhaustive mode ──────────────────────────────────────────────────────────


class TestExhaustiveMode:
    """Tests for exhaustive=True scanning behaviour."""

    def test_exhaustive_returns_multiple_on_ambiguous(self):
        """With genuinely ambiguous data, exhaustive=True returns all matching formats."""
        fmts = infer_ts.infer_format(["01/02/2024", "03/04/2024"], exhaustive=True)
        assert len(fmts) == 2
        assert "%d/%m/%Y" in fmts
        assert "%m/%d/%Y" in fmts

    def test_exhaustive_scans_all_values(self):
        """exhaustive=True continues past a unique match, catching later contradictions."""
        # Non-exhaustive: exits after first value narrows to one format
        fmts = infer_ts.infer_format(["2024-01-15T10:30:00+05:30", "not-a-timestamp"])
        assert fmts == ["%Y-%m-%dT%H:%M:%S%:z"]

        # Exhaustive: second value eliminates all formats → empty
        fmts = infer_ts.infer_format(
            ["2024-01-15T10:30:00+05:30", "not-a-timestamp"], exhaustive=True
        )
        assert fmts == []


# ─── RFC 2822 date-only ───────────────────────────────────────────────────────


class TestRfc2822DateOnly:
    """RFC 2822 date-only format (no time component)."""

    def test_rfc2822_date_only_roundtrip(self):
        s = pl.Series("ts", ["Mon, 15 Jan 2024", "Thu, 20 Jun 2024"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime
        v = _dt(result, 0)
        assert (v.year, v.month, v.day) == (2024, 1, 15)

    def test_rfc2822_date_only_format_string(self):
        fmts = infer_ts.infer_format(["Mon, 15 Jan 2024"])
        assert fmts == ["%a, %d %b %Y"]


# ─── Spaced timezone ──────────────────────────────────────────────────────────


class TestSpacedTimezone:
    """Timestamps with a space between the time component and timezone offset."""

    def test_space_sep_spaced_tz_offset(self):
        s = pl.Series("ts", ["2024-01-15 10:30:00 +05:30", "2024-06-20 08:00:00 +02:00"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime

    def test_space_sep_spaced_tz_compact(self):
        s = pl.Series("ts", ["2024-01-15 10:30:00 +0530", "2024-06-20 08:00:00 +0200"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime

    def test_t_sep_spaced_tz_compact(self):
        s = pl.Series("ts", ["2024-01-15T10:30:00 +0530", "2024-06-20T08:00:00 +0200"])
        result = infer_ts.to_datetime(s)

        assert result.dtype == pl.Datetime

    def test_spaced_tz_format_strings(self):
        fmts = infer_ts.infer_format(["2024-01-15 10:30:00 +05:30"])
        assert fmts == ["%Y-%m-%d %H:%M:%S %:z"]

        fmts = infer_ts.infer_format(["2024-01-15 10:30:00 +0530"])
        assert fmts == ["%Y-%m-%d %H:%M:%S %z"]

        fmts = infer_ts.infer_format(["2024-01-15T10:30:00 +0530"])
        assert fmts == ["%Y-%m-%dT%H:%M:%S %z"]


# ─── Timezone handling ───────────────────────────────────────────────────────


class TestTimezoneHandling:
    """Regression tests for tz-offset inputs (bug: tz was silently dropped)."""

    def test_offset_series_dtype_is_utc(self):
        """Series path: tz-offset input → datetime[μs, UTC]."""
        s = pl.Series("ts", ["2026-02-24T10:00:00+01:00"])
        result = infer_ts.to_datetime(s)
        assert result.dtype == pl.Datetime("us", "UTC")

    def test_offset_series_value_is_utc_converted(self):
        """Series path: +01:00 offset → value is wall-clock minus 1 hour."""
        s = pl.Series("ts", ["2026-02-24T10:00:00+01:00"])
        result = infer_ts.to_datetime(s)
        v = _dt(result, 0)
        assert (v.year, v.month, v.day, v.hour, v.minute) == (2026, 2, 24, 9, 0)

    def test_offset_expr_dtype_is_utc(self):
        """Expression plugin: tz-offset input → datetime[μs, UTC]."""
        df = pl.DataFrame({"ts": ["2026-02-24T10:00:00+01:00"]})
        result = df.with_columns(infer_ts.to_datetime("ts"))["ts"]
        assert result.dtype == pl.Datetime("us", "UTC")

    def test_offset_expr_value_is_utc_converted(self):
        """Expression plugin: +01:00 offset → value is wall-clock minus 1 hour."""
        df = pl.DataFrame({"ts": ["2026-02-24T10:00:00+01:00"]})
        result = df.with_columns(infer_ts.to_datetime("ts"))["ts"]
        v = _dt(result, 0)
        assert (v.year, v.month, v.day, v.hour, v.minute) == (2026, 2, 24, 9, 0)

    def test_compact_offset_expr(self):
        """Compact offset (+0100) also converts correctly."""
        df = pl.DataFrame({"ts": ["2026-02-24T10:00:00+0100"]})
        result = df.with_columns(infer_ts.to_datetime("ts"))["ts"]
        assert result.dtype == pl.Datetime("us", "UTC")
        v = _dt(result, 0)
        assert (v.hour, v.minute) == (9, 0)

    def test_naive_unaffected(self):
        """tz-naive inputs still produce timezone-naive datetime."""
        df = pl.DataFrame({"ts": ["2026-02-24T10:00:00"]})
        result = df.with_columns(infer_ts.to_datetime("ts"))["ts"]
        assert result.dtype == pl.Datetime("us", None)

    def test_series_and_expr_consistent(self):
        """Series and expression paths return the same dtype and value."""
        values = ["2026-02-24T10:00:00+01:00", "2026-06-15T08:30:00-05:00"]
        series_result = infer_ts.to_datetime(pl.Series("ts", values))
        expr_result = pl.DataFrame({"ts": values}).with_columns(infer_ts.to_datetime("ts"))["ts"]
        assert series_result.dtype == expr_result.dtype
        assert series_result.to_list() == expr_result.to_list()


if __name__ == "__main__":
    _ = pytest.main([__file__, "-v"])
