"""Tests for the Polars expression plugin.

Covers both the namespace style (pl.col("ts").infer_ts.to_datetime()) and
the functional style (infer_ts.to_datetime(pl.col("ts"))).
"""

import polars as pl
import pytest

import infer_ts

from .conftest import _dt

# ─── Namespace: to_datetime ──────────────────────────────────────────────────


class TestToDatetime:
    """pl.col("ts").infer_ts.to_datetime() in eager and lazy contexts."""

    def test_eager_with_columns(self):
        df = pl.DataFrame({"ts": ["2024-01-15T10:30:00", "2024-06-20T08:00:00"]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime())

        assert result["ts"].dtype == pl.Datetime
        v = _dt(result["ts"], 0)
        assert (v.year, v.month, v.day) == (2024, 1, 15)
        assert (v.hour, v.minute, v.second) == (10, 30, 0)

    def test_lazy_collect(self):
        df = pl.LazyFrame({"ts": ["2024-01-15T10:30:00", "2024-06-20T08:00:00"]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime()).collect()

        assert result["ts"].dtype == pl.Datetime
        assert _dt(result["ts"], 1).month == 6

    def test_select(self):
        df = pl.DataFrame({"ts": ["2024-01-15 10:30:00"]})
        result = df.select(pl.col("ts").infer_ts.to_datetime())

        assert result["ts"].dtype == pl.Datetime
        assert _dt(result["ts"], 0).hour == 10

    def test_space_separated(self):
        df = pl.DataFrame({"ts": ["2024-01-15 10:30:00", "2024-06-20 08:00:00"]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime())

        assert result["ts"].dtype == pl.Datetime
        v = _dt(result["ts"], 0)
        assert (v.year, v.month, v.day) == (2024, 1, 15)

    def test_date_only(self):
        df = pl.DataFrame({"ts": ["2024-01-15", "2024-06-20"]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime())

        assert result["ts"].dtype == pl.Datetime
        v = _dt(result["ts"], 0)
        assert (v.year, v.month, v.day) == (2024, 1, 15)
        assert (v.hour, v.minute, v.second) == (0, 0, 0)

    def test_slash_us(self):
        df = pl.DataFrame({"ts": ["01/15/2024", "06/20/2024"]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime())

        assert result["ts"].dtype == pl.Datetime
        assert _dt(result["ts"], 0).month == 1
        assert _dt(result["ts"], 0).day == 15


# ─── Unix epoch ──────────────────────────────────────────────────────────────


class TestUnixEpoch:
    """Unix epoch timestamps via plugin."""

    def test_unix_seconds(self):
        df = pl.DataFrame({"ts": ["1705312200", "1705398600"]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime())

        assert result["ts"].dtype == pl.Datetime
        v = _dt(result["ts"], 0)
        assert v.year == 2024

    def test_unix_ms(self):
        df = pl.DataFrame({"ts": ["1705312200000", "1705398600000"]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime())

        assert result["ts"].dtype == pl.Datetime
        v = _dt(result["ts"], 0)
        assert v.year == 2024

    def test_unix_us(self):
        df = pl.DataFrame({"ts": ["1705312200000000"]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime())

        assert result["ts"].dtype == pl.Datetime
        assert _dt(result["ts"], 0).year == 2024

    def test_unix_ns(self):
        df = pl.DataFrame({"ts": ["1705312200000000000"]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime())

        assert result["ts"].dtype == pl.Datetime
        assert _dt(result["ts"], 0).year == 2024


# ─── time_unit parameter ────────────────────────────────────────────────────


class TestTimeUnit:
    """to_datetime(time_unit=...) produces the correct Datetime dtype."""

    def test_default_is_us(self):
        df = pl.DataFrame({"ts": ["2024-01-15T10:30:00"]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime())
        assert result["ts"].dtype == pl.Datetime("us")

    def test_ms(self):
        df = pl.DataFrame({"ts": ["2024-01-15T10:30:00"]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime(time_unit="ms"))
        assert result["ts"].dtype == pl.Datetime("ms")
        v = _dt(result["ts"], 0)
        assert (v.year, v.month, v.day) == (2024, 1, 15)
        assert (v.hour, v.minute, v.second) == (10, 30, 0)

    def test_ns(self):
        df = pl.DataFrame({"ts": ["2024-01-15T10:30:00"]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime(time_unit="ns"))
        assert result["ts"].dtype == pl.Datetime("ns")
        v = _dt(result["ts"], 0)
        assert (v.year, v.month, v.day) == (2024, 1, 15)

    def test_us_explicit(self):
        df = pl.DataFrame({"ts": ["2024-01-15T10:30:00"]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime(time_unit="us"))
        assert result["ts"].dtype == pl.Datetime("us")

    def test_invalid_time_unit(self):
        df = pl.DataFrame({"ts": ["2024-01-15T10:30:00"]})
        with pytest.raises(ValueError, match="time_unit must be"):
            df.with_columns(pl.col("ts").infer_ts.to_datetime(time_unit="s"))  # ty: ignore[invalid-argument-type]

    def test_unix_seconds_ms(self):
        df = pl.DataFrame({"ts": ["1705312200", "1705398600"]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime(time_unit="ms"))
        assert result["ts"].dtype == pl.Datetime("ms")
        assert _dt(result["ts"], 0).year == 2024

    def test_functional_time_unit_ns(self):
        df = pl.DataFrame({"ts": ["2024-01-15T10:30:00"]})
        result = df.with_columns(infer_ts.to_datetime(pl.col("ts"), time_unit="ns"))
        assert result["ts"].dtype == pl.Datetime("ns")


# ─── Error handling ──────────────────────────────────────────────────────────


class TestErrors:
    """Error conditions."""

    def test_raise_on_multiple(self):
        df = pl.DataFrame({"ts": ["01/02/2024", "03/04/2024"]})
        with pytest.raises(Exception, match="multiple timestamp formats"):
            df.with_columns(pl.col("ts").infer_ts.to_datetime())

    def test_date_preference_eu(self):
        df = pl.DataFrame({"ts": ["01/02/2024", "03/04/2024"]})
        result = df.with_columns(
            pl.col("ts").infer_ts.to_datetime(raise_on_multiple=False, date_preference="eu")
        )
        assert result["ts"].dtype == pl.Datetime
        assert _dt(result["ts"], 0).month == 2  # EU: 01/02 → Feb

    def test_date_preference_us(self):
        df = pl.DataFrame({"ts": ["01/02/2024", "03/04/2024"]})
        result = df.with_columns(
            pl.col("ts").infer_ts.to_datetime(raise_on_multiple=False, date_preference="us")
        )
        assert result["ts"].dtype == pl.Datetime
        assert _dt(result["ts"], 0).month == 1  # US: 01/02 → Jan

    def test_no_raise_on_multiple(self):
        df = pl.DataFrame({"ts": ["01/02/2024", "03/04/2024"]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime(raise_on_multiple=False))
        assert result["ts"].dtype == pl.Datetime

    def test_all_null(self):
        # NoData: no non-null values — should produce a null Datetime series,
        # not raise an error.
        df = pl.DataFrame({"ts": [None, None, None]}, schema={"ts": pl.String})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime())

        assert result["ts"].dtype == pl.Datetime
        assert result["ts"].null_count() == 3

    def test_unrecognized_format_raises(self):
        # NoMatch: non-null values are present but no format matches.
        # Before this fix, to_datetime() silently returned a null series.
        # After this fix, it raises a ComputeError.
        df = pl.DataFrame({"ts": ["not a timestamp", "also not a timestamp"]})
        with pytest.raises(pl.exceptions.ComputeError, match="no timestamp format matched"):
            df.with_columns(pl.col("ts").infer_ts.to_datetime())

    def test_mixed_null_and_unrecognized_raises(self):
        # Non-null garbage mixed with nulls should still raise, not silently
        # return nulls.
        df = pl.DataFrame({"ts": [None, "garbage", None]}, schema={"ts": pl.String})
        with pytest.raises(pl.exceptions.ComputeError, match="no timestamp format matched"):
            df.with_columns(pl.col("ts").infer_ts.to_datetime())

    def test_all_empty_strings(self):
        # Empty strings are trimmed and skipped just like nulls (NoData),
        # so the result is a null Datetime series, not an error.
        df = pl.DataFrame({"ts": ["", "", ""]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime())
        assert result["ts"].dtype == pl.Datetime
        assert result["ts"].null_count() == 3

    def test_all_whitespace_strings(self):
        # Whitespace-only strings are also trimmed to empty and skipped (NoData).
        df = pl.DataFrame({"ts": ["   ", "\t", "\n"]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime())
        assert result["ts"].dtype == pl.Datetime
        assert result["ts"].null_count() == 3

    def test_empty_strings_mixed_with_valid(self):
        # Empty and whitespace-only strings are skipped; valid timestamps are parsed.
        df = pl.DataFrame({"ts": ["", "2024-01-15T10:30:00", "   "]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime())
        assert result["ts"].dtype == pl.Datetime
        assert result["ts"].null_count() == 2
        assert _dt(result["ts"], 1).year == 2024


# ─── infer_ts.to_datetime() series API ──────────────────────────────────────


class TestSeriesAPI:
    """infer_ts.to_datetime() series-level API."""

    def test_time_unit_us(self):
        s = pl.Series(["2024-01-15T10:30:00", "2024-06-20T08:00:00"])
        result = infer_ts.to_datetime(s, time_unit="us")
        assert result.dtype == pl.Datetime("us")
        assert result[0].year == 2024  # type: ignore[union-attr]

    def test_time_unit_ns(self):
        s = pl.Series(["2024-01-15T10:30:00", "2024-06-20T08:00:00"])
        result = infer_ts.to_datetime(s, time_unit="ns")
        assert result.dtype == pl.Datetime("ns")

    def test_time_unit_ms(self):
        s = pl.Series(["2024-01-15T10:30:00"])
        result = infer_ts.to_datetime(s, time_unit="ms")
        assert result.dtype == pl.Datetime("ms")

    def test_unix_seconds_time_unit_ns(self):
        s = pl.Series(["1705312200", "1705398600"])
        result = infer_ts.to_datetime(s, time_unit="ns")
        assert result.dtype == pl.Datetime("ns")
        assert result[0].year == 2024  # type: ignore[union-attr]

    def test_unix_ns_time_unit_ms(self):
        # nanoseconds down-scaled to ms
        s = pl.Series(["1705312200000000000"])
        result = infer_ts.to_datetime(s, time_unit="ms")
        assert result.dtype == pl.Datetime("ms")
        assert result[0].year == 2024  # type: ignore[union-attr]


# ─── Unix overflow ───────────────────────────────────────────────────────────


class TestUnixOverflow:
    """Non-strict Unix scaling: overflow values become null, not silently wrong dates.

    i64 nanoseconds saturate at 9_223_372_036_854_775_807, corresponding to
    9_223_372_036 Unix seconds (~year 2262). Scaling beyond this must not wrap.
    """

    def test_plugin_seconds_to_ns_overflow_is_null(self):
        """Seconds too large to represent as ns → null, not a wrapped wrong date."""
        # 9_999_999_999 * 1_000_000_000 > i64::MAX
        df = pl.DataFrame({"ts": ["9999999999"]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime(time_unit="ns", strict=False))
        assert result["ts"].dtype == pl.Datetime("ns")
        assert result["ts"][0] is None

    def test_series_api_seconds_to_ns_overflow_is_null(self):
        """Same overflow via infer_ts.to_datetime()."""
        s = pl.Series(["9999999999"])
        result = infer_ts.to_datetime(s, time_unit="ns", strict=False)
        assert result.dtype == pl.Datetime("ns")
        assert result[0] is None

    def test_plugin_seconds_to_ns_boundary_is_valid(self):
        """Largest Unix second that fits in ns (9_223_372_036 ≈ year 2262) is valid."""
        df = pl.DataFrame({"ts": ["9223372036"]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime(time_unit="ns"))
        assert result["ts"].dtype == pl.Datetime("ns")
        assert result["ts"][0] is not None
        assert _dt(result["ts"], 0).year == 2262

    def test_plugin_seconds_past_boundary_is_null(self):
        """One second past the ns boundary → null."""
        df = pl.DataFrame({"ts": ["9223372037"]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime(time_unit="ns", strict=False))
        assert result["ts"].dtype == pl.Datetime("ns")
        assert result["ts"][0] is None

    def test_plugin_mixed_overflow_and_valid(self):
        """Valid values are preserved alongside overflow values (which become null)."""
        df = pl.DataFrame({"ts": ["1705312200", "9999999999"]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime(time_unit="ns", strict=False))
        assert result["ts"].dtype == pl.Datetime("ns")
        assert result["ts"][0] is not None
        assert _dt(result["ts"], 0).year == 2024
        assert result["ts"][1] is None


# ─── Functional style ───────────────────────────────────────────────────────


class TestFunctionalStyle:
    """infer_ts.to_datetime() with Expr and str inputs."""

    def test_to_datetime_expr(self):
        df = pl.DataFrame({"ts": ["2024-01-15T10:30:00", "2024-06-20T08:00:00"]})
        result = df.with_columns(infer_ts.to_datetime(pl.col("ts")))

        assert result["ts"].dtype == pl.Datetime
        v = _dt(result["ts"], 0)
        assert (v.year, v.month, v.day) == (2024, 1, 15)

    def test_to_datetime_expr_lazy(self):
        df = pl.LazyFrame({"ts": ["2024-01-15T10:30:00"]})
        result = df.with_columns(infer_ts.to_datetime(pl.col("ts"))).collect()

        assert result["ts"].dtype == pl.Datetime

    def test_to_datetime_str(self):
        df = pl.DataFrame({"ts": ["2024-01-15T10:30:00", "2024-06-20T08:00:00"]})
        result = df.with_columns(infer_ts.to_datetime("ts"))

        assert result["ts"].dtype == pl.Datetime
        v = _dt(result["ts"], 0)
        assert (v.year, v.month, v.day) == (2024, 1, 15)


# ─── Dot-separated EU and month-name formats ─────────────────────────────────


class TestDotAndMonthFormats:
    """Plugin path for dot-separated EU and month-name date formats."""

    def test_dot_eu_date(self):
        df = pl.DataFrame({"ts": ["15.01.2024", "20.06.2024"]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime())

        assert result["ts"].dtype == pl.Datetime
        v = _dt(result["ts"], 0)
        assert (v.year, v.month, v.day) == (2024, 1, 15)

    def test_dot_eu_datetime(self):
        df = pl.DataFrame({"ts": ["15.01.2024 10:30:00", "20.06.2024 08:00:00"]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime())

        assert result["ts"].dtype == pl.Datetime
        assert _dt(result["ts"], 0).hour == 10

    def test_month_name_us(self):
        df = pl.DataFrame({"ts": ["Jan 15, 2024", "Jun 20, 2024"]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime())

        assert result["ts"].dtype == pl.Datetime
        v = _dt(result["ts"], 0)
        assert (v.month, v.day) == (1, 15)

    def test_month_name_eu(self):
        df = pl.DataFrame({"ts": ["15 Jan 2024", "20 Jun 2024"]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime())

        assert result["ts"].dtype == pl.Datetime
        v = _dt(result["ts"], 0)
        assert (v.month, v.day) == (1, 15)

    def test_month_name_us_datetime(self):
        df = pl.DataFrame({"ts": ["Jan 15, 2024 10:30:00", "Jun 20, 2024 08:00:00"]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime())

        assert result["ts"].dtype == pl.Datetime
        assert _dt(result["ts"], 0).hour == 10


# ─── RFC 2822 via plugin ──────────────────────────────────────────────────────


class TestRfc2822Plugin:
    """RFC 2822 datetime and date-only formats via the expression plugin."""

    def test_rfc2822_datetime(self):
        df = pl.DataFrame(
            {"ts": ["Mon, 15 Jan 2024 10:30:00 +0530", "Thu, 20 Jun 2024 08:00:00 -0800"]}
        )
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime())

        assert result["ts"].dtype == pl.Datetime
        v = _dt(result["ts"], 0)
        assert (v.month, v.day) == (1, 15)

    def test_rfc2822_date_only(self):
        df = pl.DataFrame({"ts": ["Mon, 15 Jan 2024", "Thu, 20 Jun 2024"]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime())

        assert result["ts"].dtype == pl.Datetime
        v = _dt(result["ts"], 0)
        assert (v.month, v.day) == (1, 15)
