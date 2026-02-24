from __future__ import annotations

from collections.abc import Iterable
from typing import Literal, overload

import polars as pl

__version__: str

def infer_format(
    values: pl.Series | Iterable[str | None],
    *,
    exhaustive: bool = False,
) -> list[str]: ...
@overload
def to_datetime(
    values: pl.Series,
    *,
    exhaustive: bool = ...,
    raise_on_multiple: bool = ...,
    time_unit: Literal["ns", "us", "ms"] = ...,
    date_preference: Literal["eu", "us"] = ...,
) -> pl.Series: ...
@overload
def to_datetime(
    values: pl.Expr | str,
    *,
    exhaustive: bool = ...,
    raise_on_multiple: bool = ...,
    time_unit: Literal["ns", "us", "ms"] = ...,
    date_preference: Literal["eu", "us"] = ...,
) -> pl.Expr: ...
def to_datetime(
    values: pl.Series | pl.Expr | str,
    *,
    exhaustive: bool = ...,
    raise_on_multiple: bool = ...,
    time_unit: Literal["ns", "us", "ms"] = ...,
    date_preference: Literal["eu", "us"] = ...,
) -> pl.Series | pl.Expr: ...

class ExprInferTsNamespace:
    def __init__(self, expr: pl.Expr) -> None: ...
    def to_datetime(
        self,
        *,
        exhaustive: bool = ...,
        raise_on_multiple: bool = ...,
        time_unit: Literal["ns", "us", "ms"] = ...,
        date_preference: Literal["eu", "us"] = ...,
    ) -> pl.Expr: ...
