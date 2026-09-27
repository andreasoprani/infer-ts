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
        self._expr: pl.Expr = expr

    def to_datetime(
        self,
        *,
        strict: bool = True,
        raise_on_multiple: bool = True,
        time_unit: Literal["ns", "us", "ms"] = "us",
        date_preference: Literal["eu", "us"] = "eu",
    ) -> pl.Expr:
        """Infer timestamp format and cast to Datetime.

        Args:
            strict: If *True* (default), raise on failed conversions of non-null,
                non-blank values. If *False*, return null for failed conversions.
                Nulls and whitespace-only strings are accepted in either mode.
                Inference always uses early exit and can still fail in either mode.
            raise_on_multiple: If *True* (default), error when multiple formats
                match.  If *False*, use the first match.
            time_unit: Output datetime time unit. One of ``"ns"``, ``"us"``,
                ``"ms"``. Defaults to ``"us"`` (microseconds).
            date_preference: Which slash date convention to prefer when the data
                is ambiguous (i.e. all day/month values ≤ 12 so both ``DD/MM``
                and ``MM/DD`` are plausible).  One of ``"eu"`` (default) or
                ``"us"``.  **Only takes effect when** ``raise_on_multiple=False``
                — with the default strict mode the ambiguity is still reported
                as an error.  Has no effect when the data unambiguously resolves
                to one format.
        """
        return to_datetime_expr(
            self._expr,
            strict=strict,
            raise_on_multiple=raise_on_multiple,
            time_unit=time_unit,
            date_preference=date_preference,
        )
