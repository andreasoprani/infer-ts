"""Type stubs for the infer_ts Rust extension module."""

__version__: str

def infer_format(
    values: list[str | None],
    exhaustive: bool = False,
) -> list[str]:
    """Infer timestamp format(s) from a string column using constraint elimination.

    Args:
        values: A list of string values or None. None entries are skipped.
        exhaustive: If True, process all values and return all compatible formats.
                    If False (default), return as soon as only one format remains.

    Returns:
        A list of Polars-compatible format strings, e.g. ["%Y-%m-%dT%H:%M:%S"].
        For Unix epoch columns the return values include special markers like
        "@unix_seconds", "@unix_ms", "@unix_us", "@unix_ns".

    Special cases:
        - Empty list: no format matches all values
        - All formats: column contains only nulls/empty values
        - Multiple formats: ambiguous input (e.g., US vs EU date format)
    """
    ...

def supported_formats() -> list[tuple[str, str]]:
    """Return all supported timestamp formats as (name, polars_format) pairs.

    Returns:
        A list of tuples where each tuple contains:
        - name: Human-readable format name
        - polars_format: Polars-compatible format string
    """
    ...
