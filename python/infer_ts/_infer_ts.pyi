"""Type stubs for the native Rust extension module."""

__version__: str

def infer_format(
    values: list[str | None],
    exhaustive: bool = False,
) -> list[str]: ...
