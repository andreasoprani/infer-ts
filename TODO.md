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
  - `Format::all()` generates all 104 possible combinations (4 dates × 2 seps × 3 times × 4 tz + 4 date-only + 4 unix)
  - Validators reject impossible combinations (e.g., slash dates with `T` separator)
  - Removed constraint methods (`valid_separators`, `valid_time_formats`, `valid_timezones`)
  - Removed `name()` methods — only `polars_format()` matters for inference
  - Removed named constants — tests use helper functions instead
  - Adding a new date family now requires only: add variant, implement `polars_date()` fragment, update validator to check combination validity
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
