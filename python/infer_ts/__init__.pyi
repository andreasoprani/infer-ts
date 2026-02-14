"""Type stubs for infer_ts."""

import polars as pl

__version__: str

def infer_format(
    values: list[str | None] | pl.Series,
    *,
    exhaustive: bool = False,
) -> list[str]:
    """Infer timestamp format(s) from a string column using constraint elimination.

    Accepts either a Python list of strings or a Polars Series.
    When a Series is passed, uses zero-copy Arrow access for efficiency.

    Args:
        values: A list of string values (or None) or a Polars Series.
        exhaustive: If True, process all values and return all compatible formats.
                    If False (default), return as soon as only one format remains.

    Returns:
        A list of Polars-compatible format strings, e.g. ["%Y-%m-%dT%H:%M:%S"].
        For Unix epoch columns the return values include special markers like
        "@unix_seconds", "@unix_ms", "@unix_us", "@unix_ns".
    """
    ...

def to_datetime(
    series: pl.Series,
    *,
    format: str | None = None,
    exhaustive: bool = False,
    raise_on_multiple: bool = True,
) -> pl.Series:
    """Infer timestamp format and cast a string Series to Datetime.

    Args:
        series: A Polars Series of strings to parse.
        format: Optional format string (strftime or @unix_* marker).
                If None, infers from values.
        exhaustive: Passed to infer_format when format is None.
        raise_on_multiple: If True (default), raise ValueError when multiple
                           formats match. If False, use the first match.

    Returns:
        A Polars Series with Datetime (or Date) dtype.

    Raises:
        ValueError: No format matches, or (if raise_on_multiple) ambiguous.
    """
    ...
