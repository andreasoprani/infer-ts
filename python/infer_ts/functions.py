"""Polars expression plugin functions for infer-ts.

These use ``register_plugin_function`` to call the Rust ``#[polars_expr]``
functions compiled into the same ``.so`` as the PyO3 extension.
"""

from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING

from polars.plugins import register_plugin_function

if TYPE_CHECKING:
    from typing import Literal

    import polars as pl

PLUGIN_PATH = Path(__file__).parent

__all__: list[str] = []


def to_datetime_expr(
    expr: pl.Expr,
    *,
    strict: bool = True,
    raise_on_multiple: bool = True,
    time_unit: Literal["ns", "us", "ms"] = "us",
    date_preference: Literal["eu", "us"] = "eu",
) -> pl.Expr:
    """Infer timestamp format and cast a string column to Datetime.

    This is the functional form of ``pl.col("ts").infer_ts.to_datetime()``.

    Args:
        expr: A Polars expression producing a String column.
        strict: If *True* (default), raise on failed conversions of non-null,
            non-blank values. If *False*, return null for failed conversions.
            Nulls and whitespace-only strings are accepted in either mode.
            Inference always uses early exit and can still fail in either mode.
        raise_on_multiple: If *True* (default), error when multiple formats
            match.  If *False*, use the first match.
        time_unit: Output datetime time unit. One of ``"ns"``, ``"us"``,
            ``"ms"``. Defaults to ``"us"`` (microseconds).
        date_preference: Which slash date convention to prefer when the data is
            ambiguous (i.e. all day/month values ≤ 12 so both ``DD/MM`` and
            ``MM/DD`` are plausible).  One of ``"eu"`` (default) or ``"us"``.
            **Only takes effect when** ``raise_on_multiple=False`` — with the
            default strict mode the ambiguity is still reported as an error.
            Has no effect when the data unambiguously resolves to one format.

    Returns:
        A Polars expression producing a Datetime column.
    """
    if time_unit not in ("ns", "us", "ms"):
        raise ValueError(f"time_unit must be 'ns', 'us', or 'ms', got {time_unit!r}")
    return register_plugin_function(
        plugin_path=PLUGIN_PATH,
        function_name=f"to_datetime_expr_{time_unit}",
        args=[expr],
        is_elementwise=False,
        kwargs={
            "strict": strict,
            "raise_on_multiple": raise_on_multiple,
            "date_preference": date_preference,
        },
    )
