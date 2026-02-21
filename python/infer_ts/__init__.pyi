from __future__ import annotations

from collections.abc import Iterable
from typing import Literal

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
    exhaustive: bool = False,
    raise_on_multiple: bool = True,
    time_unit: Literal["ns", "us", "ms"] = "us",
) -> pl.Series: ...
def to_datetime_expr(
    expr: pl.Expr,
    *,
    exhaustive: bool = False,
    raise_on_multiple: bool = True,
    time_unit: Literal["ns", "us", "ms"] = "us",
) -> pl.Expr: ...
