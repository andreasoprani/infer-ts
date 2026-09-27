"""Conversion strictness is independent of inference and accepts missing values."""

from __future__ import annotations

from collections.abc import Sequence
from datetime import datetime
from typing import TYPE_CHECKING, Literal, Protocol, TypedDict

if TYPE_CHECKING:
    from typing import Unpack

import polars as pl
import pytest

import infer_ts


class ConversionOptions(TypedDict, total=False):
    strict: bool
    raise_on_multiple: bool
    time_unit: Literal["ns", "us", "ms"]
    date_preference: Literal["eu", "us"]


class Converter(Protocol):
    def __call__(
        self,
        values: pl.Series | Sequence[str | None],
        **kwargs: Unpack[ConversionOptions],
    ) -> pl.Series: ...


@pytest.fixture(params=["series", "column", "expr", "namespace", "lazy"])
def convert(request: pytest.FixtureRequest) -> Converter:
    def run(
        values: pl.Series | Sequence[str | None],
        **kwargs: Unpack[ConversionOptions],
    ) -> pl.Series:
        series = (
            values if isinstance(values, pl.Series) else pl.Series("ts", values, dtype=pl.String)
        )
        if request.param == "series":
            return infer_ts.to_datetime(series, **kwargs)
        df = series.to_frame()
        if request.param == "namespace":
            expr = pl.col("ts").infer_ts.to_datetime(**kwargs)
        elif request.param == "column":
            expr = infer_ts.to_datetime("ts", **kwargs)
        else:
            expr = infer_ts.to_datetime(pl.col("ts"), **kwargs)
        if request.param == "lazy":
            return df.lazy().with_columns(expr).collect()["ts"]
        return df.with_columns(expr)["ts"]

    return run


@pytest.mark.parametrize("strict", [None, True])
@pytest.mark.parametrize("bad", ["not a timestamp", "2026-02-30T00:00:00"])
def test_strict_raises_after_early_exit(convert: Converter, strict: bool | None, bad: str) -> None:
    kwargs: ConversionOptions = {} if strict is None else {"strict": strict}
    with pytest.raises(pl.exceptions.ComputeError, match="conversion failed at row 1"):
        convert(["2026-01-13T00:00:00", bad], **kwargs)


def test_non_strict_returns_null(convert: Converter) -> None:
    result = convert(["2026-01-13T00:00:00", "invalid"], strict=False)
    assert result.to_list() == [datetime(2026, 1, 13), None]


@pytest.mark.parametrize("strict", [True, False])
@pytest.mark.parametrize("value", ["2026-01-13T00:00:00", "1768262400"])
def test_missing_values_are_accepted(convert: Converter, strict: bool, value: str) -> None:
    values = [None, "", " \t\n", "\u2003", value, "", None, "   "]
    result = convert(values, strict=strict)
    assert result.null_count() == len(values) - 1
    assert result[4] is not None
    assert result.name == "ts"


@pytest.mark.parametrize("strict", [True, False])
@pytest.mark.parametrize("values", [[], [None, None], ["", " \t\n", "\u2003", None]])
def test_no_data(convert: Converter, strict: bool, values: list[str | None]) -> None:
    result = convert(values, strict=strict, time_unit="ns")
    assert result.dtype == pl.Datetime("ns")
    assert len(result) == len(values)
    assert result.null_count() == len(values)


@pytest.mark.parametrize("value", ["9999999999", "9223372037"])
def test_strict_unix_scaling_overflow(convert: Converter, value: str) -> None:
    with pytest.raises(pl.exceptions.ComputeError, match="conversion failed"):
        convert([value], time_unit="ns")
    assert convert([value], time_unit="ns", strict=False).to_list() == [None]


def test_unix_invalid_integer_after_early_exit(convert: Converter) -> None:
    values = ["1705312200", "invalid", "99999999999999999999999"]
    with pytest.raises(pl.exceptions.ComputeError, match="conversion failed at row 1"):
        convert(values)
    result = convert(values, strict=False)
    assert result[0] is not None
    assert result.to_list()[1:] == [None, None]


@pytest.mark.parametrize("strict", [True, False])
def test_inference_failure_still_raises(convert: Converter, strict: bool) -> None:
    with pytest.raises(pl.exceptions.ComputeError, match="no timestamp format matched"):
        convert(["invalid", "2026-01-13T00:00:00"], strict=strict)


@pytest.mark.parametrize("strict", [True, False])
def test_ambiguity_policy_is_independent(convert: Converter, strict: bool) -> None:
    values = ["01/02/2026", "03/04/2026"]
    with pytest.raises(pl.exceptions.ComputeError, match="multiple timestamp formats"):
        convert(values, strict=strict)
    result = convert(values, strict=strict, raise_on_multiple=False, date_preference="us")
    assert result.to_list() == [datetime(2026, 1, 2), datetime(2026, 3, 4)]


def test_multichunk_missing_values_and_failure(convert: Converter) -> None:
    series = pl.concat(
        [
            pl.Series("ts", [None, "2026-01-13T00:00:00"]),
            pl.Series("ts", ["", "   ", None]),
            pl.Series("ts", ["2026-01-14T00:00:00", "invalid"]),
        ],
        rechunk=False,
    )
    assert series.n_chunks() == 3
    with pytest.raises(pl.exceptions.ComputeError, match="conversion failed at row 6"):
        convert(series)
    result = convert(series, strict=False)
    assert result.null_count() == 5
    assert result[5] == datetime(2026, 1, 14)


def test_conversion_no_longer_accepts_exhaustive(convert: Converter) -> None:
    with pytest.raises(TypeError, match="exhaustive"):
        # Deliberately pass the removed keyword to verify runtime rejection.
        convert(["2026-01-13T00:00:00"], exhaustive=True)  # pyright: ignore[reportCallIssue]


def test_infer_format_keeps_exhaustive() -> None:
    values = ["2026-01-13T00:00:00", "invalid"]
    assert infer_ts.infer_format(values) == ["%Y-%m-%dT%H:%M:%S"]
    assert infer_ts.infer_format(values, exhaustive=True) == []
