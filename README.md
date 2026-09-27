# infer-ts

Infer timestamp formats from string columns using **CSP constraint elimination**, built in Rust via [PyO3](https://pyo3.rs/) for use with Python and [Polars](https://pola-rs.github.io/polars/).

## Why this library?

Polars can parse string columns to `Datetime` via `str.to_datetime(format=...)`, but requires you to supply the format string. When no format is given (`format=None`), Polars infers it — but with two significant limitations:

**Only ISO 8601 variants are reliably inferred.** Common real-world formats fail entirely with `format=None`:

```python
pl.Series(["01/15/2024"]).str.to_datetime()
# ComputeError: could not find an appropriate format to parse dates,
# please define a format

pl.Series(["1705312200"]).str.to_datetime()   # Unix timestamps
pl.Series(["Jan 15, 2024"]).str.to_datetime() # Month-name dates
pl.Series(["20240115T103000"]).str.to_datetime() # Compact dates
# All raise the same error
```

**Format is inferred from the first non-null value only.** For slash dates, Polars always assumes day-first (`%d/%m/%Y`). If your data is US-formatted (`%m/%d/%Y`), you either get silently wrong dates or a parse error on the first value where `day > 12`:

```python
# Polars locks on %d/%m/%Y from the first row, then fails on month=15
pl.Series(["03/04/2024", "01/15/2024"]).str.to_datetime()
# ComputeError: … failed for 1 out of 2 values: ["01/15/2024"]
```

The alternative — wrapping `str.to_datetime` in a `try/except` loop over candidate formats — is verbose, slow, and still only tells you the first format that works on the first value.

**infer-ts** solves this by scanning the entire column once with a CSP algorithm that tracks all compatible formats simultaneously. It returns every format consistent with the full column, handles ambiguity explicitly (e.g. reporting both `%d/%m/%Y` and `%m/%d/%Y` when the data doesn't distinguish them), and supports dozens of formats that Polars cannot auto-infer.

## How it works

The inference engine treats each candidate timestamp format as a variable in a Constraint Satisfaction Problem (CSP). Each cell value in the column acts as a constraint that narrows the candidate set:

1. **Initialise** – start with all format combinations as candidates.
2. **Propagate** – for each non-null cell, eliminate every format that cannot parse that value.
3. **Early exit** (default) – as soon as a single format remains, return it immediately. For `infer_format`, set `exhaustive=True` to disable this. Conversion always uses early-exit inference.
4. **Return** – return all formats that survived constraint propagation.

Inference can stop once the format is unambiguous. Use `infer_format(..., exhaustive=True)` to validate format consistency across the input. `to_datetime` instead uses `strict=True` by default to reject failed conversions, without an exhaustive inference pass.

## Supported formats

See **[FORMATS.md](FORMATS.md)** for the full reference tables (date patterns, time patterns, timezone suffixes, and Unix epoch formats with their Polars format strings). That file is auto-generated from the Rust source — to regenerate after changing any format definitions:

```sh
cargo test -- --ignored dump_formats
```

### Architecture: Compositional format design

Internally, formats are represented using a compositional structure rather than a flat enum:

```
Format
├── Date { date: DateFmt }
├── DateTime { date: DateFmt, sep: Separator, time: TimeFmt, tz: Option<Timezone>, spaced_tz: bool }
└── Unix { precision: UnixPrecision }
```

All combinations of the above components are tried automatically. Structurally invalid ones (e.g. a value whose first character isn't `T` or space at the separator position) are eliminated by the parser on the first value, so the performance cost is negligible.

Adding a new format variant (e.g. named timezones) requires a single new enum variant and a validator — no changes to the combinatorial logic.

## Installation

Install the Python package with pip:

```sh
pip install infer-ts
```

Then import it as `infer_ts`:

```python
import infer_ts
```

If you are installing from a local checkout instead of PyPI, use:

```sh
pip install .
```

## Development

Development requires [Rust](https://www.rust-lang.org/), [maturin](https://github.com/PyO3/maturin), and [uv](https://github.com/astral-sh/uv) for reproducible installs.

```sh
uv sync --all-extras
uv run maturin develop --release
bash build-and-test.sh
```

## Usage

### Quick start

```python
import polars as pl
import infer_ts

df = pl.DataFrame({"ts": ["2024-01-15T10:30:00", "2024-06-20T08:00:00"]})

# Series → Series
series = infer_ts.to_datetime(df["ts"])

# Column name → Expr  (works inside lazy frames too)
df = df.with_columns(infer_ts.to_datetime("ts"))

# Expr → Expr
df = df.with_columns(infer_ts.to_datetime(pl.col("ts")))

# Namespace style (equivalent to the Expr form above)
df = df.with_columns(pl.col("ts").infer_ts.to_datetime())

# Control the output time unit (default: "us")
df = df.with_columns(infer_ts.to_datetime("ts", time_unit="ns"))

# Infer the format strings only (returns a list of all matching formats)
fmts = infer_ts.infer_format(df["ts"])
```

### Strict conversion and missing values

`to_datetime` accepts `strict=True` (the default), not `exhaustive`. This applies
identically to Series, column names, expressions, and the expression namespace.

```python
s = pl.Series("ts", ["01/13/2026 00:00:00", "invalid"])

infer_ts.to_datetime(s)
# raises ComputeError: conversion failed ...

infer_ts.to_datetime(s, strict=False)
# [2026-01-13 00:00:00, null]
```

- Nulls, empty strings, and whitespace-only strings are treated as missing values in both modes. All-missing inputs produce an all-null Datetime Series.
- Strict conversion rejects failed non-blank conversions, including Unix integer parsing and scaling overflow. Non-strict conversion returns nulls for them.
- Inference always uses early exit. `strict=False` does **not** make inference tolerant of arbitrary garbage: if no format can be inferred (for example, the first non-blank value is invalid), conversion still raises.
- Ambiguity is independent of strictness: `raise_on_multiple=True` still raises when multiple formats survive. Use `raise_on_multiple=False` and, optionally, `date_preference="eu"` or `"us"` to choose an interpretation.
- On clean data, strict conversion checks only null counts. If conversion creates new nulls, it inspects the input/output to distinguish blank strings from errors; it does not parse the timestamps again.

### Basic inference

```python
import infer_ts

# Returns a list of compatible formats
fmts = infer_ts.infer_format([
    "2024-01-15T10:30:00",
    "2024-06-20T08:00:00",
    None,                       # nulls are skipped
])
print(fmts)       # ["%Y-%m-%dT%H:%M:%S"]
print(fmts[0])    # "%Y-%m-%dT%H:%M:%S"

# Use exhaustive=True to process all values
fmts = infer_ts.infer_format(["01/02/2024", "03/04/2024"], exhaustive=True)
print(fmts)  # ["%d/%m/%Y", "%m/%d/%Y"] - both US and EU formats match
```

### Polars – using the inferred format directly

If you need the format string itself (e.g. to pass to other tools), use `infer_format` and call `str.to_datetime` manually:

```python
import polars as pl
import infer_ts

df = pl.DataFrame({"ts": ["2024-01-15T10:30:00", "2024-06-20T08:00:00"]})

fmts = infer_ts.infer_format(df["ts"])
# Use first format (or handle multiple if ambiguous)
fmt = fmts[0]
df = df.with_columns(pl.col("ts").str.to_datetime(format=fmt))
```

### Polars – Unix epoch formats

`infer_format` returns a `@`-prefixed marker for epoch columns (e.g. `@unix_seconds`, `@unix_ms`, `@unix_us`, `@unix_ns`). `to_datetime` handles these automatically with correct integer scaling — no manual casting needed:

```python
import polars as pl
import infer_ts

df = pl.DataFrame({"ts": ["1705312200", "1705398600"]})

df = df.with_columns(infer_ts.to_datetime("ts"))       # Expr form
result = infer_ts.to_datetime(df["ts"])                # Series form
```

## Handling ambiguity

US and EU slash dates are inherently ambiguous when every day value is ≤ 12. Instead of raising an error, the library returns all compatible formats:

```python
import infer_ts

# Ambiguous – both mm/dd and dd/mm interpretations are valid for every row
fmts = infer_ts.infer_format(["01/02/2024", "03/04/2024"])
print(fmts)       # ["%d/%m/%Y", "%m/%d/%Y"]
print(len(fmts))  # 2 - caller can choose or prompt user

# Resolved – day 15 > 12 eliminates the US interpretation
fmts = infer_ts.infer_format(["01/02/2024", "15/03/2024"])
print(fmts)  # ["%d/%m/%Y"] - uniquely EU

# No match returns empty list
fmts = infer_ts.infer_format(["not a timestamp"])
print(fmts)  # []
```

## Contributing

This project was developed with heavy use of LLM agents (primarily Claude) for both the initial implementation and subsequent refinement. If you spot a bug, an edge case the parser mishandles, or a timestamp format that should be supported, issues and pull requests are very welcome.
