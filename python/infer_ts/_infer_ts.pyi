"""Type stubs for the native Rust extension module."""

from collections.abc import Iterator

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
    iter: Iterator[str | None],
    *,
    exhaustive: bool = False,
) -> list[str]: ...
