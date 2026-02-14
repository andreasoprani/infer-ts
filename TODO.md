# TODO

Items are grouped by release milestone. Work through them roughly top-to-bottom;
later sections depend on earlier ones being solid.

---

## v0.2 – Polars format string verification

- [x] Verify `%:z` offset parsing works in Polars `str.to_datetime`
  - Both `%:z` (colon) and `%z` (no colon) work in Polars
  - Split offset formats into colon (`+05:30` → `%:z`) and compact (`+0530` → `%z`) variants
  - Added `Iso8601DateTimeOffsetCompact` and `Iso8601DateTimeFracOffsetCompact` formats
- [x] **Compositional format architecture** (`src/formats.rs`)
  - Refactored from flat 21-variant enum to compositional structure
  - Component enums: `DateFmt`, `Separator`, `TimeFmt`, `Timezone`, `UnixPrecision`
  - `StandardFormat` enum with `DateOnly` and `DateTime` variants
  - `UnixFormat` struct for epoch timestamps
  - Top-level `Format` enum: `Standard(StandardFormat) | Unix(UnixFormat)`
- [x] **Programmatic format enumeration** (`src/formats.rs`)
  - ~~`Format::all()` generates all combinations~~ (replaced by `Format::parse()` in v0.4)
  - Removed constraint methods (`valid_separators`, `valid_time_formats`, `valid_timezones`)
  - Removed `name()` methods — only `polars_format()` matters for inference
  - Removed named constants — tests use helper functions instead
  - Adding a new date family now requires only: add variant, implement `polars_date()` fragment, add `parse_date` branch
- [x] Verify `%.f` fractional-second handling in Polars
  - `%.f` works correctly with 1–9 fractional digits and mixed precisions in one column
  - 9-digit values truncate to microseconds (Polars' default Datetime resolution) — expected
  - No fixed-width specifiers needed; no changes required
- [x] Write a Python convenience wrapper that handles the `@unix_*` markers transparently – takes a Polars Series and returns a cast Series regardless of whether the inferred format is string-based or epoch-based

---

## v0.3 – Additional format families

- [x] Space-separated datetime with timezone: `2024-01-15 10:30:00+05:30`
- [x] 12-hour time with AM/PM: `01/15/2024 2:30:00 PM`
- [x] Short (2-digit) year: `01/15/24`, `15/01/24`
- [x] Dot-separated European date: `15.01.2024`
- [x] Month-name formats: `Jan 15, 2024` / `15 Jan 2024`
- [x] RFC 2822 / email style: `Mon, 15 Jan 2024 10:30:00 +0530`

---

## v0.4 – Compositional parsing refactoring

**See**: [REFACTORING_PLAN.md](./REFACTORING_PLAN.md) for detailed step-by-step plan

- [x] **Replace explicit validation with compositional parsing**
  - Removed 200+ line `validates()` method with nested match statements
  - Implemented `StandardFormat::parse()` that composes date/separator/time/timezone linearly
  - Added `TimeComponent` struct grouping separator + time format + timezone
  - Added component parsers in `date.rs`: `parse_date`, `parse_separator`, `parse_time`, `parse_frac`, `parse_timezone`
  - Accepts all structurally valid combinations (removed artificial restrictions)
  - Removed ~30 monolithic validator functions (~550 lines)
- [x] **Lazy domain initialization for inference**
  - Removed `Format::all()` and `OnceLock` — no more pre-generating format combinations
  - Added `Format::parse()` for on-demand format discovery from a value
  - Hybrid approach: parse first value to seed candidates, validate subsequent values
  - Removed `supported_formats()` Python API (no longer meaningful)
  - Empty/all-null columns now return `[]` (can't infer from no data)
- [x] **Update documentation**
  - Updated module-level docs to describe compositional parsing and lazy domain init
  - Removed outdated comments about pre-enumeration and CSP constraint elimination
  - Updated Python type stubs, `__init__.py`, and README
- [x] **Validation & testing**
  - All 191 Rust tests pass, 69 Python integration tests pass
  - 0 clippy warnings, formatting clean
  - 3 former rejection tests updated to acceptance tests (compositional parser correctly accepts more combinations: T separator + AM/PM, AM/PM + timezone, compact date + AM/PM)

## v0.4.1 – Minor changes

- [x] Fix spaced timezone support
  - Generalized to all date formats (not just RFC 2822): added `spaced_tz: bool` to `TimeComponent`
  - Parser now tries both spaced and non-spaced timezone for all formats
  - `polars_format()` emits space before tz when `spaced_tz=true`
- [x] Deterministic formats order (when returning multiple formats)
  - Derived `PartialOrd`/`Ord` on all format types
  - Inference results sorted before returning
- [x] Verify that all-nulls series is correctly cast as all-null datetime series
  - `infer_format()` returns empty list for all-nulls (can't infer from no data)
  - `to_datetime()` detects all-null series and returns an all-null `pl.Datetime` column
  - Added Python tests verifying both behaviors
- [x] Rename `std` utility function in test → `dt()`
  - Also renamed `std` match variables to `sf` in non-test code
- [x] Evaluate if Standard -> polars_format can return str instead of String
  - Can't return `&str` (format string is composed dynamically)
  - Made `UnixFormat::polars_format()` return `String` for API consistency
- [x] Add `parse()` to `UnixFormat` to match `StandardFormat`
  - `Format::parse()` now delegates to both `StandardFormat::parse()` and `UnixFormat::parse()`
- [x] ~~Maybe separate DateFmt and StandardFormat~~ — evaluated, not needed
  - Current `StandardFormat` enum with `DateOnly`/`DateTime` variants already provides this distinction
- [x] Check that `parse_iso_date` is actually compliant with ISO
  - Correctly enforces `YYYY-MM-DD` (4-digit year, leading zeros). Extended year not supported but irrelevant.
- [x] Add failure tests for edge-cases
  - Added 17 new tests: trailing garbage, partial/invalid timezones, zero month/day, Feb 30/31, double timezone, non-numeric parts, unix digit bounds
- [x] Remove restriction on Rfc2822 always requiring the time part
  - `Mon, 15 Jan 2024` now parses as date-only Rfc2822
- [x] `StandardFormat.validates()` now validates single format directly
  - Composes component parsers for only the specific format's components instead of parsing all formats

---

## v0.5 – Performance

- [x] Accept a Polars Series directly via PyO3 (avoid the Python list → `Vec` copy for large columns)
  - Added `pyo3-polars` (0.25) + `polars` (0.52) dependencies; upgraded pyo3 0.22→0.26
  - New `infer_format_series()` Rust function accepts `PySeries` for zero-copy Arrow access
  - Generalized `infer()` to accept `impl IntoIterator<Item = Option<&str>>`
  - Python `infer_format()` dispatches to series path when given a `pl.Series`
  - `to_datetime()` uses series path directly (no more `series.to_list()`)
  - Requires `pyarrow` at runtime for pyo3-polars Series conversion
- [x] Streaming / iterator interface for columns that don't fit in memory
  - Refactored inference engine into `InferState` struct with `feed()` + `finish()` methods
  - `infer()` is now a thin wrapper over `InferState`
  - New `infer_format_iter()` Rust function accepts any Python iterable, pulling values lazily
  - Python `infer_format()` dispatches generators/iterables to the streaming path
  - Early-exit works across the Python boundary — generator stops being consumed once format is resolved
- [x] Benchmark on large columns (1 M+ rows) to confirm early-exit behaviour and lazy dataframes support

---

## v0.6 – Polars plugin (single-pass infer + cast)

- [ ] Expose as a Polars expression plugin: `df.with_columns(pl.col("ts").infer_ts.to_datetime())`, simple setup with two passes, infer and then cast.
- [ ] **Single-pass datetime parsing**
  - Infer format and cast to datetime in one pass over the data
  - Avoid the current two-pass approach (infer → cast)
- [ ] **Format hint parameter**
  - Accept an optional format hint to skip inference when format is known
  - Useful for performance when format is predetermined
- [x] ~~**Architecture considerations**~~ (done in v0.2)
  - ~~Current `Vec<Format>` return type prepares for trying formats in priority order~~
  - ~~Consider returning `InferResult` struct with: formats, confidence score, error details~~
  - Compositional format architecture now in place; extending formats is straightforward

## v0.7 - Distribution

- [ ] CI pipeline: build manylinux wheels with maturin and publish to PyPI
