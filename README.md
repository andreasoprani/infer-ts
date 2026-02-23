# infer-ts

Infer timestamp formats from string columns using **CSP constraint elimination**,
built in Rust via [PyO3](https://pyo3.rs/) for use with Python and
[Polars](https://pola-rs.github.io/polars/).

_Note_: This project is in development and not yet ready for use.
It was almost entirely *vibe-coded* as an experiment and a tool for personal use.
Contributions are welcome if you spot any bugs or have suggestions for improvement.

## Why this library?

Polars can parse string columns to `Datetime` via `str.to_datetime(format=...)`, but
requires you to supply the format string. When no format is given (`format=None`),
Polars infers it — but with two significant limitations:

**Only ISO 8601 variants are reliably inferred.** Common real-world formats fail
entirely with `format=None`:

```python
pl.Series(["01/15/2024"]).str.to_datetime()
# ComputeError: could not find an appropriate format to parse dates,
# please define a format

pl.Series(["1705312200"]).str.to_datetime()   # Unix timestamps
pl.Series(["Jan 15, 2024"]).str.to_datetime() # Month-name dates
pl.Series(["20240115T103000"]).str.to_datetime() # Compact dates
# All raise the same error
```

**Format is inferred from the first non-null value only.** For slash dates, Polars
always assumes day-first (`%d/%m/%Y`). If your data is US-formatted (`%m/%d/%Y`), you
either get silently wrong dates or a parse error on the first value where `day > 12`:

```python
# Polars locks on %d/%m/%Y from the first row, then fails on month=15
pl.Series(["03/04/2024", "01/15/2024"]).str.to_datetime()
# ComputeError: … failed for 1 out of 2 values: ["01/15/2024"]
```

The alternative — wrapping `str.to_datetime` in a `try/except` loop over candidate
formats — is verbose, slow, and still only tells you the first format that works on
the first value.

**infer-ts** solves this by scanning the entire column once with a CSP algorithm that
tracks all compatible formats simultaneously. It returns every format consistent with
the full column, handles ambiguity explicitly (e.g. reporting both `%d/%m/%Y` and
`%m/%d/%Y` when the data doesn't distinguish them), and supports dozens of formats
that Polars cannot auto-infer.

## How it works

The inference engine treats each candidate timestamp format as a variable in a
Constraint Satisfaction Problem (CSP). Each cell value in the column acts as a
constraint that narrows the candidate set:

1. **Initialise** – start with all format combinations as candidates.
2. **Propagate** – for each non-null cell, eliminate every format that cannot
   parse that value.
3. **Early exit** (default) – as soon as a single format remains, return it
   immediately. Set `exhaustive=True` to disable this and check all values.
4. **Return** – return all formats that survived constraint propagation.

This approach is both efficient (resolves in the first few rows for most real
data) and flexible (use `exhaustive=True` to validate the _entire_ column).

## Supported formats

See **[FORMATS.md](FORMATS.md)** for the full reference tables (date patterns,
time patterns, timezone suffixes, and Unix epoch formats with their Polars format
strings). That file is auto-generated from the Rust source — to regenerate after
changing any format definitions:

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

All combinations of the above components are tried automatically. Structurally invalid
ones (e.g. a value whose first character isn't `T` or space at the separator position)
are eliminated by the parser on the first value, so the performance cost is negligible.

Adding a new format variant (e.g. named timezones) requires a single new enum variant and
a validator — no changes to the combinatorial logic.

## Installation

Requires [Rust](https://www.rust-lang.org/) and
[maturin](https://github.com/PyO3/maturin).

```sh
pip install maturin
maturin develop --release
```

For development (uses [uv](https://github.com/astral-sh/uv) for reproducible installs):

```sh
uv sync --all-extras
maturin develop --release
bash build-and-test.sh
```

## Usage

### Quick start

```python
import polars as pl
import infer_ts

df = pl.DataFrame({"ts": ["2024-01-15T10:30:00", "2024-06-20T08:00:00"]})

# One-liner: infer format and cast to Datetime in a single call
series = infer_ts.to_datetime(df["ts"])

# Or as a Polars expression (works inside lazy frames too)
df = df.with_columns(pl.col("ts").infer_ts.to_datetime())

# Control the output time unit (default: "us")
df = df.with_columns(pl.col("ts").infer_ts.to_datetime(time_unit="ns"))

# Infer the format strings only (returns a list of all matching formats)
fmts = infer_ts.infer_format(df["ts"])
```

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

### Polars – string-based formats

```python
import polars as pl
import infer_ts

df = pl.DataFrame({"ts": ["2024-01-15T10:30:00", "2024-06-20T08:00:00"]})

fmts = infer_ts.infer_format(df["ts"].to_list())
# Use first format (or handle multiple if ambiguous)
fmt = fmts[0]
df = df.with_columns(pl.col("ts").str.to_datetime(format=fmt))
```

### Polars – Unix epoch formats

`infer_format` returns a `@`-prefixed marker for epoch columns. These require
integer casting rather than format-string parsing:

```python
import polars as pl
import infer_ts

EPOCH_UNITS = {
    "@unix_seconds": "us",   # cast seconds → microseconds for Datetime
    "@unix_ms":      "us",
    "@unix_us":      "us",
    "@unix_ns":      "ns",
}

df = pl.DataFrame({"ts": ["1705312200", "1705398600"]})
fmts = infer_ts.infer_format(df["ts"].to_list())
fmt = fmts[0] if fmts else None

if fmt in EPOCH_UNITS:
    df = df.with_columns(
        pl.col("ts").cast(pl.Int64).cast(pl.Datetime(EPOCH_UNITS[fmt]))
    )
```

## Handling ambiguity

US and EU slash dates are inherently ambiguous when every day value is ≤ 12.
Instead of raising an error, the library returns all compatible formats:

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
