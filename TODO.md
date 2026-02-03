# TODO

Items are grouped by release milestone.  Work through them roughly top-to-bottom;
later sections depend on earlier ones being solid.

---

## v0.1 – Test coverage (next session)

- [ ] **Unit tests for every format validator** (`src/formats.rs`)
  - Happy-path: one canonical example per format
  - Boundary dates: leap years (Feb 29), month-end days, year 0 / negative years
  - Fractional-second precision: 1 digit through 9 digits
  - Timezone offsets: `+00:00`, `-12:00`, `+0530` (no-colon variant)
  - Reject cases: wrong separator, truncated string, out-of-range values
- [ ] **Unit tests for the inference engine** (`src/inference.rs`)
  - Single-format columns (one value is enough for early exit)
  - Ambiguous columns that stay ambiguous (US vs EU slash with all days ≤ 12)
  - Columns that start ambiguous and resolve mid-way
  - All-null / all-empty columns → `EmptyColumn` error
  - Mixed nulls and valid values
- [ ] **Integration test: round-trip with Polars** (Python test script)
  - Spin up a `DataFrame`, call `infer_format`, apply the returned format with
    `str.to_datetime`, assert the resulting dtype is `Datetime`
  - Cover at least: ISO 8601, space-separated, US/EU slash, compact, Unix epoch

---

## v0.2 – Polars format string verification

- [ ] Verify `%:z` offset parsing works in Polars `str.to_datetime`
  - If Polars uses `%z` (no colon) instead, update `polars_format()` accordingly
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
