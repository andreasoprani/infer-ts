"""Tests for the Polars expression plugin.

Covers both the namespace style (pl.col("ts").infer_ts.to_datetime()) and
the functional style (infer_ts.to_datetime_expr("ts")).
"""

from datetime import datetime

import infer_ts
import polars as pl
import pytest


def _dt(s: pl.Series, idx: int) -> datetime:
    """Extract element from series, asserting it's a datetime."""
    v = s[idx]  # pyright: ignore[reportAny]
    assert isinstance(v, datetime)
    return v


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
            df.with_columns(pl.col("ts").infer_ts.to_datetime(time_unit="s"))

    def test_unix_seconds_ms(self):
        df = pl.DataFrame({"ts": ["1705312200", "1705398600"]})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime(time_unit="ms"))
        assert result["ts"].dtype == pl.Datetime("ms")
        assert _dt(result["ts"], 0).year == 2024

    def test_functional_time_unit_ns(self):
        df = pl.DataFrame({"ts": ["2024-01-15T10:30:00"]})
        result = df.with_columns(
            infer_ts.to_datetime_expr(pl.col("ts"), time_unit="ns")
        )
        assert result["ts"].dtype == pl.Datetime("ns")


# ─── Error handling ──────────────────────────────────────────────────────────


class TestErrors:
    """Error conditions."""

    def test_raise_on_multiple(self):
        df = pl.DataFrame({"ts": ["01/02/2024", "03/04/2024"]})
        with pytest.raises(Exception, match="multiple timestamp formats"):
            df.with_columns(pl.col("ts").infer_ts.to_datetime())

    def test_no_raise_on_multiple(self):
        df = pl.DataFrame({"ts": ["01/02/2024", "03/04/2024"]})
        result = df.with_columns(
            pl.col("ts").infer_ts.to_datetime(raise_on_multiple=False)
        )
        assert result["ts"].dtype == pl.Datetime

    def test_all_null(self):
        df = pl.DataFrame({"ts": [None, None, None]}, schema={"ts": pl.String})
        result = df.with_columns(pl.col("ts").infer_ts.to_datetime())

        assert result["ts"].dtype == pl.Datetime
        assert result["ts"].null_count() == 3


# ─── Functional style ───────────────────────────────────────────────────────


class TestFunctionalStyle:
    """infer_ts.to_datetime_expr() functional style."""

    def test_to_datetime_expr(self):
        df = pl.DataFrame({"ts": ["2024-01-15T10:30:00", "2024-06-20T08:00:00"]})
        result = df.with_columns(infer_ts.to_datetime_expr(pl.col("ts")))

        assert result["ts"].dtype == pl.Datetime
        v = _dt(result["ts"], 0)
        assert (v.year, v.month, v.day) == (2024, 1, 15)

    def test_to_datetime_expr_lazy(self):
        df = pl.LazyFrame({"ts": ["2024-01-15T10:30:00"]})
        result = df.with_columns(infer_ts.to_datetime_expr(pl.col("ts"))).collect()

        assert result["ts"].dtype == pl.Datetime
