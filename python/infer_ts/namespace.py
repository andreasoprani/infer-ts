"""Polars expression namespace ``infer_ts``.

Importing this module registers the namespace so that
``pl.col("ts").infer_ts.to_datetime()`` works.
"""

from __future__ import annotations

from typing import Literal

import polars as pl

from infer_ts.functions import to_datetime_expr


@pl.api.register_expr_namespace("infer_ts")
class ExprInferTsNamespace:
    """Expression namespace for infer-ts timestamp inference."""

    def __init__(self, expr: pl.Expr):
        self._expr = expr

    def to_datetime(
        self,
        *,
        exhaustive: bool = False,
        raise_on_multiple: bool = True,
        time_unit: Literal["ns", "us", "ms"] = "us",
    ) -> pl.Expr:
        """Infer timestamp format and cast to Datetime.

        Args:
            exhaustive: If *True*, check all values during inference.
            raise_on_multiple: If *True*, error when multiple formats match.
            time_unit: Output datetime time unit. One of ``"ns"``, ``"us"``,
                ``"ms"``. Defaults to ``"us"`` (microseconds).
        """
        return to_datetime_expr(
            self._expr,
            exhaustive=exhaustive,
            raise_on_multiple=raise_on_multiple,
            time_unit=time_unit,
        )
