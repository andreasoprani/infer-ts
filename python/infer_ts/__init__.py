"""infer-ts: Infer timestamp formats from string columns.

Re-exports the Rust extension and adds a convenience ``to_datetime()``
wrapper that transparently handles both strftime and ``@unix_*`` formats.
"""

from __future__ import annotations

from collections.abc import Iterable
from typing import TYPE_CHECKING, Literal

from ._infer_ts import __version__
from ._infer_ts import infer_format_iter as _infer_format_iter
from ._infer_ts import infer_format_series as _infer_format_series

if TYPE_CHECKING:
    import polars as pl

import infer_ts.namespace  # noqa: F401  — registers the expr namespace  # pyright: ignore[reportUnusedImport]
from infer_ts.functions import to_datetime_expr

__all__ = [
    "__version__",
    "infer_format",
    "to_datetime",
    "to_datetime_expr",
]

# Source precision exponent (power of 10 relative to seconds) for each unix marker.
_UNIX_SOURCE_EXP: dict[str, int] = {
    "@unix_seconds": 0,
    "@unix_ms": 3,
    "@unix_us": 6,
    "@unix_ns": 9,
}

_TIME_UNIT_EXP: dict[str, int] = {"ms": 3, "us": 6, "ns": 9}

_EU_SLASH = "%d/%m/"
_US_SLASH = "%m/%d/"


def _apply_date_preference(fmts: list[str], date_preference: str) -> list[str]:
    """When both EU and US slash formats are present, keep only the preferred one."""
    has_eu = any(_EU_SLASH in f for f in fmts)
    has_us = any(_US_SLASH in f for f in fmts)
    if not (has_eu and has_us):
        return fmts
    drop = _US_SLASH if date_preference != "us" else _EU_SLASH
    return [f for f in fmts if drop not in f]


def infer_format(
    values: pl.Series | Iterable[str | None],
    *,
    exhaustive: bool = False,
) -> list[str]:
    """Infer timestamp format(s) from a string column.

    Accepts a Polars Series or any iterable of strings (list, generator, etc.).
    Series uses zero-copy Arrow access; iterables are consumed lazily (streaming),
    so the full column never needs to be in memory at once.
    """
    import polars as pl

    if isinstance(values, pl.Series):
        return _infer_format_series(values, exhaustive=exhaustive)
    return _infer_format_iter(iter(values), exhaustive=exhaustive)


def to_datetime(
    series: pl.Series,
    *,
    exhaustive: bool = False,
    raise_on_multiple: bool = True,
    time_unit: Literal["ns", "us", "ms"] = "us",
    date_preference: Literal["eu", "us"] = "eu",
) -> pl.Series:
    """Infer timestamp format and cast a string Series to Datetime.

    Args:
        series: A Polars Series of strings to parse.
        exhaustive: If *True*, check all values during inference.
        raise_on_multiple: If *True* (default), raise :class:`ValueError`
            when multiple formats match.  If *False*, use the first match.
        time_unit: Output datetime time unit — ``"ns"``, ``"us"``, or ``"ms"``.
            Defaults to ``"us"`` (microseconds).
        date_preference: Which slash date convention to prefer when the data is
            ambiguous (i.e. all day/month values ≤ 12 so both ``DD/MM`` and
            ``MM/DD`` are plausible).  One of ``"eu"`` (default) or ``"us"``.
            **Only takes effect when** ``raise_on_multiple=False`` — with the
            default strict mode the ambiguity is still reported as an error.
            Has no effect when the data unambiguously resolves to one format.

    Returns:
        A Polars Series with ``Datetime(time_unit)`` dtype.

    Raises:
        ValueError: No format matches, or (if *raise_on_multiple*) ambiguous.
    """
    import polars as pl

    fmts = _infer_format_series(series, exhaustive=exhaustive)

    if not fmts:
        # All-null/empty series: return an all-null Datetime series
        if series.null_count() == len(series):
            return series.cast(pl.Datetime(time_unit))
        raise ValueError("No timestamp format matches the values in the series")

    if len(fmts) > 1 and raise_on_multiple:
        raise ValueError(
            f"Multiple timestamp formats match the values: {fmts}. "
            + "Pass raise_on_multiple=False to use the first match."
        )

    fmts = _apply_date_preference(fmts, date_preference)
    fmt = fmts[0]
    if fmt in _UNIX_SOURCE_EXP:
        diff = _TIME_UNIT_EXP[time_unit] - _UNIX_SOURCE_EXP[fmt]
        ints = series.cast(pl.Int64)
        if diff > 0:
            factor = 10**diff
            overflow = ints.abs() > (2**63 - 1) // factor
            scaled = (ints * factor).set(overflow, None)
        elif diff < 0:
            scaled = ints // 10**-diff
        else:
            scaled = ints
        return scaled.cast(pl.Datetime(time_unit))

    return series.str.to_datetime(format=fmt, time_unit=time_unit)
