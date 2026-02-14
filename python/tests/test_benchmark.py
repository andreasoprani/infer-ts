"""Benchmark tests: large columns and lazy frame integration.

Verifies:
- Early-exit makes inference near-instant on large unambiguous columns
- Series path (zero-copy) is faster than the list path
- Exhaustive mode correctly processes all rows
- Lazy dataframe integration works (collect → infer)
"""

import time
from typing import Any, Callable

import infer_ts
import polars as pl
import pytest


# ─── Helpers ──────────────────────────────────────────────────────────────────

N = 1_000_000


def _time(fn: Callable[[], Any]) -> tuple[float, Any]:
    """Return (wall-clock seconds, result) for calling *fn*."""
    start = time.perf_counter()
    result = fn()
    elapsed = time.perf_counter() - start
    return elapsed, result


# ─── Test 1: Early-exit on unambiguous column (Series) ───────────────────────


class TestEarlyExitUnambiguous:
    """1M-row unambiguous ISO 8601 column — early exit should be near-instant."""

    @pytest.fixture(scope="class")
    def series(self) -> pl.Series:
        return pl.Series("ts", ["2024-01-15T10:30:00+05:30"] * N)

    def test_early_exit_is_fast(self, series: pl.Series):
        elapsed, fmts = _time(lambda: infer_ts.infer_format(series))
        assert fmts == ["%Y-%m-%dT%H:%M:%S%:z"]
        assert elapsed < 0.2, f"early-exit took {elapsed:.3f}s, expected < 200ms"

    def test_exhaustive_slower_than_early_exit(self, series: pl.Series):
        t_early, _ = _time(lambda: infer_ts.infer_format(series))
        t_exhaust, fmts = _time(
            lambda: infer_ts.infer_format(series, exhaustive=True)
        )
        assert fmts == ["%Y-%m-%dT%H:%M:%S%:z"]
        assert t_early * 10 < t_exhaust, (
            f"early-exit ({t_early:.4f}s) should be >=10× faster "
            f"than exhaustive ({t_exhaust:.4f}s)"
        )


# ─── Test 2: Early-exit on ambiguous column (Series) ─────────────────────────


class TestEarlyExitAmbiguous:
    """1M rows, all ambiguous except the last which disambiguates to EU."""

    @pytest.fixture(scope="class")
    def series(self) -> pl.Series:
        # 01/02/2024 is ambiguous (US or EU); 13/01/2024 forces EU
        values = ["01/02/2024"] * (N - 1) + ["13/01/2024"]
        return pl.Series("ts", values)

    def test_resolves_to_eu(self, series: pl.Series):
        fmts = infer_ts.infer_format(series)
        assert fmts == ["%d/%m/%Y"], f"expected EU format, got {fmts}"

    def test_must_scan_all_rows(self, series: pl.Series):
        """Non-exhaustive mode still must scan all rows to resolve ambiguity."""
        elapsed, fmts = _time(lambda: infer_ts.infer_format(series))
        assert fmts == ["%d/%m/%Y"]
        # Should take measurably longer than the unambiguous early-exit case
        assert elapsed > 0.001, f"expected full scan, but took only {elapsed:.4f}s"


# ─── Test 3: Series vs list path ─────────────────────────────────────────────


class TestSeriesVsList:
    """Series (zero-copy Arrow) path should be faster than the list path."""

    @pytest.fixture(scope="class")
    def data(self) -> tuple[pl.Series, list[str]]:
        series = pl.Series("ts", ["2024-01-15T10:30:00+05:30"] * N)
        lst = series.to_list()
        return series, lst

    def test_series_faster_than_list(
        self, data: tuple[pl.Series, list[str]]
    ):
        series, lst = data
        t_series, fmts_s = _time(
            lambda: infer_ts.infer_format(series, exhaustive=True)
        )
        t_list, fmts_l = _time(
            lambda: infer_ts.infer_format(lst, exhaustive=True)
        )
        assert fmts_s == fmts_l
        assert t_series < t_list, (
            f"Series path ({t_series:.4f}s) should be faster "
            f"than list path ({t_list:.4f}s)"
        )


# ─── Test 4: to_datetime round-trip on 1M rows ──────────────────────────────


class TestToDatetimeRoundTrip:
    """Full round-trip: infer + parse on 1M rows."""

    @pytest.fixture(scope="class")
    def series(self) -> pl.Series:
        return pl.Series("ts", ["2024-01-15T10:30:00"] * N)

    def test_round_trip_correctness(self, series: pl.Series):
        result = infer_ts.to_datetime(series)
        assert result.dtype == pl.Datetime
        assert len(result) == N
        # spot-check first and last
        assert result[0].year == 2024  # pyright: ignore[reportOptionalMemberAccess]
        assert result[0].month == 1  # pyright: ignore[reportOptionalMemberAccess]
        assert result[-1].day == 15  # pyright: ignore[reportOptionalMemberAccess]

    def test_round_trip_timing(self, series: pl.Series):
        elapsed, result = _time(lambda: infer_ts.to_datetime(series))
        assert result.dtype == pl.Datetime
        # Just a sanity check — should complete in reasonable time
        assert elapsed < 10.0, f"round-trip took {elapsed:.2f}s"


# ─── Test 5: Lazy frame integration ──────────────────────────────────────────


class TestLazyFrame:
    """Lazy frame patterns: collect-then-infer and map_batches."""

    @pytest.fixture(scope="class")
    def lazy_frame(self) -> pl.LazyFrame:
        return pl.LazyFrame(
            {"ts": ["2024-01-15T10:30:00", "2024-06-20T08:00:00"] * 500}
        )

    def test_collect_then_infer(self, lazy_frame: pl.LazyFrame):
        """Standard pattern: collect column, then infer format / to_datetime."""
        col = lazy_frame.collect().get_column("ts")
        fmts = infer_ts.infer_format(col)
        assert fmts == ["%Y-%m-%dT%H:%M:%S"]

        result = infer_ts.to_datetime(col)
        assert result.dtype == pl.Datetime
        assert len(result) == 1000

    def test_map_batches_integration(self, lazy_frame: pl.LazyFrame):
        """map_batches pattern for lazy-compatible usage."""
        result = lazy_frame.with_columns(
            pl.col("ts")
            .map_batches(infer_ts.to_datetime, return_dtype=pl.Datetime)
            .alias("parsed")
        ).collect()

        assert result["parsed"].dtype == pl.Datetime
        assert len(result) == 1000
        assert result["parsed"][0].year == 2024  # pyright: ignore[reportOptionalMemberAccess]

    def test_lazy_large_frame(self):
        """Lazy frame with 1M rows — collect column then infer."""
        lf = pl.LazyFrame({"ts": ["2024-01-15T10:30:00+05:30"] * N})
        col = lf.collect().get_column("ts")

        elapsed, fmts = _time(lambda: infer_ts.infer_format(col))
        assert fmts == ["%Y-%m-%dT%H:%M:%S%:z"]
        # Early-exit: should be fast even on 1M rows
        assert elapsed < 0.2, f"lazy-then-infer took {elapsed:.3f}s"


if __name__ == "__main__":
    _ = pytest.main([__file__, "-v"])
