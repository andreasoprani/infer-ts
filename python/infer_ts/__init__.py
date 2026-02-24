"""infer-ts: Infer timestamp formats from string columns.

Re-exports the Rust extension and adds a convenience ``to_datetime()``
wrapper that transparently handles both strftime and ``@unix_*`` formats.
"""

from __future__ import annotations

from collections.abc import Iterable
from typing import TYPE_CHECKING, Literal, TypedDict

from ._infer_ts import __version__
from ._infer_ts import infer_format_iter as _infer_format_iter
from ._infer_ts import infer_format_series as _infer_format_series

if TYPE_CHECKING:
    import polars as pl
    from polars._typing import IntoExprColumn

import infer_ts.namespace  # noqa: F401  — registers the expr namespace  # pyright: ignore[reportUnusedImport]
from infer_ts.functions import to_datetime_expr as _to_datetime_expr


class _KwArgs(TypedDict):
    exhaustive: bool
    raise_on_multiple: bool
    time_unit: Literal["ns", "us", "ms"]
    date_preference: Literal["eu", "us"]


__all__ = [
    "__version__",
    "infer_format",
    "to_datetime",
]


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
    values: IntoExprColumn,
    *,
    exhaustive: bool = False,
    raise_on_multiple: bool = True,
    time_unit: Literal["ns", "us", "ms"] = "us",
    date_preference: Literal["eu", "us"] = "eu",
) -> pl.Series | pl.Expr:
    """Infer timestamp format and cast a string column to Datetime.

    Accepts a :class:`~polars.Series`, a :class:`~polars.Expr`, or a column
    name string.  When given a Series the result is a Series; otherwise a
    Polars expression is returned (suitable for use inside
    :meth:`~polars.DataFrame.with_columns` and lazy frames).

    Args:
        values: A Polars Series, expression, or column name string.
        exhaustive: If *True*, check all values during inference.
        raise_on_multiple: If *True* (default), raise an error when multiple
            formats match.  If *False*, use the first match.
        time_unit: Output datetime time unit — ``"ns"``, ``"us"``, or ``"ms"``.
            Defaults to ``"us"`` (microseconds).
        date_preference: Which slash date convention to prefer when the data is
            ambiguous (i.e. all day/month values ≤ 12 so both ``DD/MM`` and
            ``MM/DD`` are plausible).  One of ``"eu"`` (default) or ``"us"``.
            **Only takes effect when** ``raise_on_multiple=False`` — with the
            default strict mode the ambiguity is still reported as an error.
            Has no effect when the data unambiguously resolves to one format.

    Returns:
        A :class:`~polars.Series` with ``Datetime(time_unit)`` dtype when
        *values* is a Series; a :class:`~polars.Expr` otherwise.

    Raises:
        polars.exceptions.ComputeError: No format matches, or (if *raise_on_multiple*)
            multiple formats match.
    """
    import polars as pl

    kw: _KwArgs = {
        "exhaustive": exhaustive,
        "raise_on_multiple": raise_on_multiple,
        "time_unit": time_unit,
        "date_preference": date_preference,
    }
    match values:
        case str():
            return _to_datetime_expr(pl.col(values), **kw)
        case pl.Expr():
            return _to_datetime_expr(values, **kw)
        case pl.Series():
            # Delegate to the expression plugin so both paths share the same Rust code.
            return (
                values.to_frame().select(_to_datetime_expr(pl.col(values.name), **kw)).to_series()
            )
