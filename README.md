# infer-ts

Infer timestamp formats from string columns using **CSP constraint elimination**,
built in Rust via [PyO3](https://pyo3.rs/) for use with Python and
[Polars](https://pola-rs.github.io/polars/).

_Note_: This project is in development and not yet ready for use.
It started as a vibe-coding experiment to learn Rust, PyO3, and Polars plugins.
Contributions are welcome if you spot any bugs or have suggestions for improvement.

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

| Family                       | Example                            | Polars format string      |
| ---------------------------- | ---------------------------------- | ------------------------- |
| ISO 8601 datetime            | `2024-01-15T10:30:00`              | `%Y-%m-%dT%H:%M:%S`       |
| ISO 8601 + UTC               | `2024-01-15T10:30:00Z`             | `%Y-%m-%dT%H:%M:%SZ`      |
| ISO 8601 + offset            | `2024-01-15T10:30:00+05:30`        | `%Y-%m-%dT%H:%M:%S%:z`    |
| ISO 8601 + compact offset    | `2024-01-15T10:30:00+0530`         | `%Y-%m-%dT%H:%M:%S%z`     |
| ISO 8601 + frac              | `2024-01-15T10:30:00.123456`       | `%Y-%m-%dT%H:%M:%S%.f`    |
| ISO 8601 + frac + UTC        | `2024-01-15T10:30:00.123456Z`      | `%Y-%m-%dT%H:%M:%S%.fZ`   |
| ISO 8601 + frac + offset     | `2024-01-15T10:30:00.123456+05:30` | `%Y-%m-%dT%H:%M:%S%.f%:z` |
| ISO 8601 + frac + compact tz | `2024-01-15T10:30:00.123456+0530`  | `%Y-%m-%dT%H:%M:%S%.f%z`  |
| Space datetime               | `2024-01-15 10:30:00`              | `%Y-%m-%d %H:%M:%S`       |
| Space datetime + frac        | `2024-01-15 10:30:00.123456`       | `%Y-%m-%d %H:%M:%S%.f`    |
| Date only (ISO)              | `2024-01-15`                       | `%Y-%m-%d`                |
| US slash date                | `01/15/2024`                       | `%m/%d/%Y`                |
| EU slash date                | `15/01/2024`                       | `%d/%m/%Y`                |
| US slash datetime            | `01/15/2024 10:30:00`              | `%m/%d/%Y %H:%M:%S`       |
| EU slash datetime            | `15/01/2024 10:30:00`              | `%d/%m/%Y %H:%M:%S`       |
| Compact date                 | `20240115`                         | `%Y%m%d`                  |
| Compact datetime             | `20240115T103000`                  | `%Y%m%dT%H%M%S`           |
| Unix seconds                 | `1705312200`                       | `@unix_seconds`           |
| Unix milliseconds            | `1705312200000`                    | `@unix_ms`                |
| Unix microseconds            | `1705312200000000`                 | `@unix_us`                |
| Unix nanoseconds             | `1705312200000000000`              | `@unix_ns`                |

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

Internally, formats are represented using a compositional structure rather than a flat enum. This makes adding new format variants much easier:

```
Format
├── Standard(StandardFormat)
│   ├── DateOnly { date: DateFmt }
│   └── DateTime { date: DateFmt, sep: Separator, time: TimeFmt, tz: Option<Timezone> }
└── Unix(UnixFormat { precision: UnixPrecision })

Components:
  DateFmt:       Iso | SlashUS | SlashEU | Compact
  Separator:     T | Space
  TimeFmt:       Hms | HmsFrac | HmsCompact
  Timezone:      Utc | Offset | OffsetCompact
  UnixPrecision: Seconds | Milliseconds | Microseconds | Nanoseconds
```

All possible combinations (4 dates × 2 seps × 3 times × 4 tz options = 96 DateTime + 4 DateOnly + 4 Unix = 104 total) are generated automatically. Validators reject impossible combinations (e.g., slash dates with `T` separator, space-separated with timezone). 
The CSP eliminates these invalid formats on the first value anyway, so the performance cost is negligible.

Adding a new timezone format (e.g., named timezones) requires adding one `Timezone` variant and updating the validator, instead of duplicating across all datetime combinations.

## Installation

Requires [Rust](https://www.rust-lang.org/) and
[maturin](https://github.com/PyO3/maturin).

```sh
pip install maturin
maturin develop --release
```

## Usage

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
