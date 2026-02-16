from __future__ import annotations

from collections.abc import Iterable

import polars as pl

__version__: str

def infer_format(
    values: pl.Series | Iterable[str | None],
    *,
    exhaustive: bool = False,
) -> list[str]: ...
def to_datetime(
    series: pl.Series,
    *,
    format: str | None = None,
    exhaustive: bool = False,
    raise_on_multiple: bool = True,
) -> pl.Series: ...
def to_datetime_expr(
    expr: pl.Expr,
    *,
    format: str | None = None,
    exhaustive: bool = False,
    raise_on_multiple: bool = True,
) -> pl.Expr: ...
def infer_format_expr(
    expr: pl.Expr,
    *,
    exhaustive: bool = False,
) -> pl.Expr: ...
