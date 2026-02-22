"""Type stubs for the native Rust extension module."""

from collections.abc import Iterable

import polars as pl

__version__: str

def infer_format(
    values: list[str | None],
    *,
    exhaustive: bool = False,
) -> list[str]: ...

def infer_format_series(
    series: pl.Series,
    *,
    exhaustive: bool = False,
) -> list[str]: ...

def infer_format_iter(
    iter: Iterable[str | None],
    *,
    exhaustive: bool = False,
) -> list[str]: ...
