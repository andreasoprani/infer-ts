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

Datetime formats are detected **compositionally**: a date pattern, a separator (`T` or
space), a time pattern, and an optional timezone suffix are each matched independently.
Any valid combination is recognised automatically. Date-only values (no time component)
are also detected for all date patterns.

### Date patterns

| Name | Example | Polars fragment |
| ---- | ------- | --------------- |
| ISO 8601 | `2024-01-15` | `%Y-%m-%d` |
| US slash, 4-digit year | `01/15/2024` | `%m/%d/%Y` |
| US slash, 2-digit year | `01/15/24` | `%m/%d/%y` |
| EU slash, 4-digit year | `15/01/2024` | `%d/%m/%Y` |
| EU slash, 2-digit year | `15/01/24` | `%d/%m/%y` |
| EU dot, 4-digit year | `15.01.2024` | `%d.%m.%Y` |
| EU dot, 2-digit year | `15.01.24` | `%d.%m.%y` |
| Compact | `20240115` | `%Y%m%d` |
| Month-name US, 4-digit year | `Jan 15, 2024` | `%b %d, %Y` |
| Month-name US, 2-digit year | `Jan 15, 24` | `%b %d, %y` |
| Month-name EU, 4-digit year | `15 Jan 2024` | `%d %b %Y` |
| Month-name EU, 2-digit year | `15 Jan 24` | `%d %b %y` |
| RFC 2822 | `Tue, 15 Jan 2024` | `%a, %d %b %Y` |

2-digit years are expanded using the POSIX convention: 00–68 → 2000–2068, 69–99 → 1969–1999.

### Time patterns

| Name | Example | Polars fragment |
| ---- | ------- | --------------- |
| 24-hour | `10:30:00` | `%H:%M:%S` |
| 24-hour + fractional seconds | `10:30:00.123456` | `%H:%M:%S%.f` |
| 24-hour compact | `103000` | `%H%M%S` |
| 12-hour AM/PM | `10:30:00 PM` | `%I:%M:%S %p` |
| 12-hour AM/PM compact | `10:30:00PM` | `%I:%M:%S%p` |

Date and time are joined by `T` (ISO 8601 style) or a single space.

### Timezone suffixes (optional)

| Name | Example | Polars fragment |
| ---- | ------- | --------------- |
| UTC | `Z` | `Z` |
| Offset with colon | `+05:30` | `%:z` |
| Compact offset | `+0530` | `%z` |

A space before the timezone suffix is also accepted (e.g. `10:30:00 +05:30`).

### Unix epoch formats

| Name | Example | Marker |
| ---- | ------- | ------ |
| Unix seconds | `1705312200` | `@unix_seconds` |
| Unix milliseconds | `1705312200000` | `@unix_ms` |
| Unix microseconds | `1705312200000000` | `@unix_us` |
| Unix nanoseconds | `1705312200000000000` | `@unix_ns` |

`infer_format` returns a `@`-prefixed marker for epoch columns; see
[Polars – Unix epoch formats](#polars--unix-epoch-formats) below for how to apply them.

### Unix epoch digit-count ranges

Unix formats use non-overlapping digit-count windows so the CSP can discriminate between units without ambiguity:

| Variant          | Digit count | Approx. date range      |
| ---------------- | ----------- | ----------------------- |
| UnixSeconds      | 9–10        | 1973-03-03 … 2286-11-20 |
| UnixMilliseconds | 11–13       | 1970 … 2286 (ms)        |
| UnixMicroseconds | 14–16       | 1970 … 2286 (µs)        |
| UnixNanoseconds  | 17–19       | 1677 … 2262 (ns, i64)   |

Values with 1–8 digits do not match any Unix variant; 8-digit numeric strings are handled exclusively by `DateCompact` (if the digits form a valid date).

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
