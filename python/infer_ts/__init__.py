"""infer-ts: Infer timestamp formats from string columns.

Re-exports the Rust extension and adds a convenience ``to_datetime()``
wrapper that transparently handles both strftime and ``@unix_*`` formats.
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Literal

from ._infer_ts import __version__
from ._infer_ts import infer_format as _infer_format_list
from ._infer_ts import infer_format_series as _infer_format_series

if TYPE_CHECKING:
    import polars as pl

__all__ = [
    "__version__",
    "infer_format",
    "to_datetime",
]

# Mapping from @unix_* marker → (Polars time_unit, multiplier to reach that unit)
_EPOCH_UNITS: dict[str, tuple[Literal["ms", "us", "ns"], int]] = {
    "@unix_seconds": ("ms", 1000),
    "@unix_ms": ("ms", 1),
    "@unix_us": ("us", 1),
    "@unix_ns": ("ns", 1),
}


def infer_format(
    values: list[str | None] | pl.Series,
    *,
    exhaustive: bool = False,
) -> list[str]:
    """Infer timestamp format(s) from a string column.

    Accepts either a Python list of strings or a Polars Series.
    When a Series is passed, uses zero-copy Arrow access for efficiency.
    """
    try:
        import polars as pl

        if isinstance(values, pl.Series):
            return _infer_format_series(values, exhaustive=exhaustive)
    except ImportError:
        pass
    return _infer_format_list(values, exhaustive=exhaustive)


def to_datetime(
    series: pl.Series,
    *,
    format: str | None = None,
    exhaustive: bool = False,
    raise_on_multiple: bool = True,
) -> pl.Series:
    """Infer timestamp format and cast a string Series to Datetime.

    Args:
        series: A Polars Series of strings to parse.
        format: Optional format string (strftime or ``@unix_*`` marker).
                If *None*, infers from values.
        exhaustive: Passed to :func:`infer_format` when *format* is None.
        raise_on_multiple: If *True* (default), raise :class:`ValueError`
            when multiple formats match.  If *False*, use the first match.

    Returns:
        A Polars Series with Datetime (or Date) dtype.

    Raises:
        ValueError: No format matches, or (if *raise_on_multiple*) ambiguous.
    """
    import polars as pl

    if format is None:
        fmts = _infer_format_series(series, exhaustive=exhaustive)

        if not fmts:
            # All-null/empty series: return an all-null Datetime series
            if series.null_count() == len(series):
                return series.cast(pl.Datetime)
            raise ValueError("No timestamp format matches the values in the series")

        if len(fmts) > 1 and raise_on_multiple:
            raise ValueError(
                f"Multiple timestamp formats match the values: {fmts}. "
                + "Pass raise_on_multiple=False to use the first match, "
                + "or pass format= explicitly."
            )

        fmt = fmts[0]
    else:
        fmt = format

    if fmt in _EPOCH_UNITS:
        unit, multiplier = _EPOCH_UNITS[fmt]
        return (series.cast(pl.Int64) * multiplier).cast(pl.Datetime(unit))

    return series.str.to_datetime(format=fmt)
