# TODO

Items are grouped by release milestone. Work through them roughly top-to-bottom;
later sections depend on earlier ones being solid.

---

## v0.1 – Test coverage

- [x] **Unit tests for every format validator** (`src/formats.rs`)
  - Happy-path: one canonical example per format
  - Boundary dates: leap years (Feb 29), month-end days, year 0 / negative years
  - Fractional-second precision: 1 digit through 9 digits
  - Timezone offsets: `+00:00`, `-12:00`, `+0530` (no-colon variant)
  - Reject cases: wrong separator, truncated string, out-of-range values
- [x] **Unit tests for the inference engine** (`src/inference.rs`)
  - Single-format columns (one value is enough for early exit)
  - Ambiguous columns that stay ambiguous (US vs EU slash with all days ≤ 12)
  - Columns that start ambiguous and resolve mid-way
  - All-null / all-empty columns → returns all formats
  - Mixed nulls and valid values
- [x] **Integration test: round-trip with Polars** (Python test script)
  - Spin up a `DataFrame`, call `infer_format`, apply the returned format with
    `str.to_datetime`, assert the resulting dtype is `Datetime`
  - Cover at least: ISO 8601, space-separated, US/EU slash, compact, Unix epoch

---

## v0.1.1 – Multi-format return and exhaustive mode

- [x] **Change return type to list of formats**
  - `infer_format()` now returns `list[str]` instead of `str`
  - Ambiguous columns return all compatible formats instead of error
  - No format match returns empty list instead of error
- [x] **Add `exhaustive` parameter**
  - Default `False`: early-exit when single format remains
  - `True`: process all values, return all compatible formats
- [x] **Simplify error handling**
  - Removed `EmptyColumn` error (empty columns return all formats)
  - Removed `Ambiguous` error (return multiple formats in list)
  - Removed `NoMatch` error (return empty list)
  - Function never raises exceptions

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
  - Named constants (`Format::Iso8601DateTime`, etc.) for backwards compatibility
- [ ] **Programmatic format enumeration** (`src/formats.rs`)
  - Current limitation: adding a new `DateFmt` still requires manually adding named
    constants and updating `Format::all()`, `name()`, `polars_format()` for each combination
  - Goal: generate all valid format combinations automatically from component enums
  - Implementation steps:
    1. Add `DateFmt::all()`, `UnixPrecision::all()` iterator methods
    2. Add `DateFmt::valid_separators()` → which separators work with this date format
       - `Iso` → `[T, Space]`
       - `SlashUS`, `SlashEU` → `[Space]`
       - `Compact` → `[T]`
    3. Add `DateFmt::valid_time_formats()` → which time formats work with this date format
       - `Iso` → `[Hms, HmsFrac]`
       - `SlashUS`, `SlashEU` → `[Hms]`
       - `Compact` → `[HmsCompact]`
    4. Add `DateFmt::valid_timezones()` → which tz options work with this date format
       - `Iso` → `[None, Some(Utc), Some(Offset), Some(OffsetCompact)]`
       - `SlashUS`, `SlashEU`, `Compact` → `[None]`
    5. Add `StandardFormat::all_valid()` → iterate over all valid combinations using above methods
    6. Change `Format::all()` to use `OnceLock<Vec<Format>>` and generate from
       `StandardFormat::all_valid()` + `UnixPrecision::all()`
    7. Make `name()` and `polars_format()` compositional (build from component methods)
       - Each component implements its own `name()` and `polars_format()` fragment
       - `StandardFormat` assembles fragments: `date.fmt() + sep.fmt() + time.fmt() + tz.fmt()`
       - Return type changes from `&'static str` to `String` (minor allocation trade-off)
    8. Named constants become optional conveniences, no longer source of truth
  - Result: adding new `DateFmt::Dot` requires only:
    1. Add enum variant
    2. Implement `valid_separators()`, `valid_time_formats()`, `valid_timezones()`
    3. Implement `name()` and `polars_format()` fragments
    4. All combinations auto-generated
- [ ] Verify `%.f` fractional-second handling in Polars
  - Polars may require fixed-width specifiers like `%.3f` / `%.6f` / `%.9f`
  - If so, add fractional-precision detection to the validator and the format enum
- [ ] Write a Python convenience wrapper that handles the `@unix_*` markers
      transparently – takes a Polars Series and returns a cast Series regardless of
      whether the inferred format is string-based or epoch-based

---

## v0.3 – Additional format families

- [ ] Space-separated datetime with timezone: `2024-01-15 10:30:00+05:30`
- [ ] 12-hour time with AM/PM: `01/15/2024 2:30:00 PM`
- [ ] Short (2-digit) year: `01/15/24`, `15/01/24`
- [ ] Dot-separated European date: `15.01.2024`
- [ ] Month-name formats: `Jan 15, 2024` / `15 Jan 2024`
- [ ] RFC 2822 / email style: `Mon, 15 Jan 2024 10:30:00 +0530`
- [ ] Low-digit Unix seconds (1–8 digits, pre-1973)
  - Currently excluded to avoid ambiguity with compact dates
  - Consider a priority / tiebreaker system or a caller-supplied hint

---

## v0.4 – Performance & distribution

- [ ] Benchmark on large columns (1 M+ rows) to confirm early-exit behaviour
- [ ] Accept a Polars Series directly via PyO3 (avoid the Python list → `Vec`
      copy for large columns) – requires the `polars` PyO3 bindings or a buffer
      protocol path
- [ ] Streaming / iterator interface for columns that don't fit in memory
- [ ] CI pipeline: build manylinux wheels with maturin and publish to PyPI

---

## v0.5 – Polars plugin (single-pass infer + cast)

- [ ] **Single-pass datetime parsing**
  - Infer format and cast to datetime in one pass over the data
  - Avoid the current two-pass approach (infer → cast)
  - Expose as a Polars expression plugin: `df.with_columns(pl.col("ts").infer_ts.to_datetime())`
- [ ] **Format hint parameter**
  - Accept an optional format hint to skip inference when format is known
  - Useful for performance when format is predetermined
- [x] ~~**Architecture considerations**~~ (done in v0.2)
  - ~~Current `Vec<Format>` return type prepares for trying formats in priority order~~
  - ~~Consider returning `InferResult` struct with: formats, confidence score, error details~~
  - Compositional format architecture now in place; extending formats is straightforward
