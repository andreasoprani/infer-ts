"""Polars expression namespace ``infer_ts``.

Importing this module registers the namespace so that
``pl.col("ts").infer_ts.to_datetime()`` works.
"""

from __future__ import annotations

import polars as pl

from infer_ts.functions import infer_format_expr, to_datetime_expr


@pl.api.register_expr_namespace("infer_ts")
class ExprInferTsNamespace:
    """Expression namespace for infer-ts timestamp inference."""

    def __init__(self, expr: pl.Expr):
        self._expr = expr

    def to_datetime(
        self,
        *,
        format: str | None = None,
        exhaustive: bool = False,
        raise_on_multiple: bool = True,
    ) -> pl.Expr:
        """Infer timestamp format and cast to Datetime.

        Args:
            format: Optional format hint to skip inference.
            exhaustive: If *True*, check all values during inference.
            raise_on_multiple: If *True*, error when multiple formats match.
        """
        return to_datetime_expr(
            self._expr,
            format=format,
            exhaustive=exhaustive,
            raise_on_multiple=raise_on_multiple,
        )

    def infer_format(self, *, exhaustive: bool = False) -> pl.Expr:
        """Infer the timestamp format of a string column.

        Returns a scalar String column containing the format string.

        Args:
            exhaustive: If *True*, check all values during inference.
        """
        return infer_format_expr(self._expr, exhaustive=exhaustive)
