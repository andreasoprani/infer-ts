"""Shared test helpers for the infer-ts test suite."""

from datetime import date, datetime

import polars as pl

__all__ = ["_dt", "_d"]


def _dt(s: pl.Series, idx: int) -> datetime:
    """Extract element from a Datetime series, asserting it's a datetime."""
    v = s[idx]  # pyright: ignore[reportAny]
    assert isinstance(v, datetime)
    return v


def _d(s: pl.Series, idx: int) -> date:
    """Extract element from a Date or Datetime series, asserting it's a date."""
    v = s[idx]  # pyright: ignore[reportAny]
    assert isinstance(v, date)
    return v
