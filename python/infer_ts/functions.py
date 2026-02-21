"""Polars expression plugin functions for infer-ts.

These use ``register_plugin_function`` to call the Rust ``#[polars_expr]``
functions compiled into the same ``.so`` as the PyO3 extension.
"""

from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING

from polars.plugins import register_plugin_function

if TYPE_CHECKING:
    import polars as pl

PLUGIN_PATH = Path(__file__).parent

__all__ = ["to_datetime_expr", "infer_format_expr"]


def to_datetime_expr(
    expr: pl.Expr,
    *,
    exhaustive: bool = False,
    raise_on_multiple: bool = True,
    time_unit: str = "us",
) -> pl.Expr:
    """Infer timestamp format and cast a string column to Datetime.

    This is the functional form of ``pl.col("ts").infer_ts.to_datetime()``.

    Args:
        expr: A Polars expression producing a String column.
        exhaustive: If *True*, check all values during inference.
        raise_on_multiple: If *True*, error when multiple formats match.
        time_unit: Output datetime time unit. One of ``"ns"``, ``"us"``,
            ``"ms"``. Defaults to ``"us"`` (microseconds).

    Returns:
        A Polars expression producing a Datetime column.
    """
    if time_unit not in ("ns", "us", "ms"):
        raise ValueError(
            f"time_unit must be 'ns', 'us', or 'ms', got {time_unit!r}"
        )
    return register_plugin_function(
        plugin_path=PLUGIN_PATH,
        function_name=f"to_datetime_expr_{time_unit}",
        args=[expr],
        is_elementwise=False,
        kwargs={
            "exhaustive": exhaustive,
            "raise_on_multiple": raise_on_multiple,
        },
    )


def infer_format_expr(
    expr: pl.Expr,
    *,
    exhaustive: bool = False,
) -> pl.Expr:
    """Infer the timestamp format of a string column.

    Returns a scalar String Series containing the inferred format string.

    Args:
        expr: A Polars expression producing a String column.
        exhaustive: If *True*, check all values during inference.

    Returns:
        A Polars expression producing a String column with the format.
    """
    return register_plugin_function(
        plugin_path=PLUGIN_PATH,
        function_name="infer_format_expr",
        args=[expr],
        is_elementwise=False,
        kwargs={"exhaustive": exhaustive},
    )
