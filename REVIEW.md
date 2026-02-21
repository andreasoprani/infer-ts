# Codebase Review – infer-ts

> Generated: 2026-02-21

## Overview

**infer-ts** is a Rust timestamp-format inference engine exposed to Python via PyO3, with a
Polars expression plugin. The core algorithm is a Constraint Satisfaction Problem (CSP) that
eliminates candidate formats as values are fed through. The project is well-structured,
well-tested, and production-quality internally — the main gaps are documentation, a few
API inconsistencies, and the not-yet-done v0.7 distribution milestone.

---

## 1. Bugs / Correctness Issues

### 1.1 `__version__` hardcoded in `src/lib.rs`

`src/lib.rs:116` hardcodes `"0.1.0"` instead of reading from Cargo.toml:

```rust
m.add("__version__", "0.1.0")?;  // ← hardcoded
```

**Fix:** use `env!("CARGO_PKG_VERSION")` so it stays in sync automatically.

---

### 1.2 `infer_format_expr` silently picks the first format on ambiguity

`src/expressions.rs:121–126`:

```rust
let result = if formats.is_empty() {
    ...
} else {
    let fmt_str = formats[0].polars_format();   // ← always picks [0]
    Series::new(series.name().clone(), &[fmt_str])
};
```

There is no `raise_on_multiple` parameter here, unlike `to_datetime_expr` which does raise
on ambiguity by default. A user calling `.infer_ts.infer_format()` on a column of
`["01/02/2024", "03/04/2024"]` will silently get `%d/%m/%Y` with no warning that
`%m/%d/%Y` also matched.

**Fix:** add a `raise_on_multiple` kwarg to `infer_format_expr` (Rust + Python), consistent
with `to_datetime_expr`.

---

### 1.3 `to_datetime()` (series-level API) missing `time_unit` parameter

`python/infer_ts/__init__.py:63–104` — the `to_datetime()` convenience function hardcodes
the epoch time unit via `_EPOCH_UNITS` and always produces `ms` or `us` Datetime:

```python
_EPOCH_UNITS = {
    "@unix_seconds": ("ms", 1000),
    "@unix_ms":      ("ms", 1),
    "@unix_us":      ("us", 1),
    "@unix_ns":      ("ns", 1),
}
```

Meanwhile `to_datetime_expr()` (plugin API) fully exposes `time_unit` as `"ns"`, `"us"`,
or `"ms"`. A user who calls `infer_ts.to_datetime(series, time_unit="ns")` will get a
`TypeError` because the kwarg doesn't exist.

**Fix:** add `time_unit: Literal["ns", "us", "ms"] = "us"` to `to_datetime()` and use it
both for the string format path (`series.str.to_datetime(time_unit=time_unit)`) and the
Unix epoch path (pass it to `scale_unix` logic).

---

### 1.4 `_infer_ts.pyi` stub uses positional args instead of keyword-only

`python/infer_ts/_infer_ts.pyi` declares:

```python
def infer_format(values: list[str | None], exhaustive: bool = False) -> list[str]: ...
```

The actual PyO3 signature is `#[pyo3(signature = (values, exhaustive=false))]` — Polars
convention (and the Python wrapper) treats `exhaustive` as keyword-only. The stub should
reflect this:

```python
def infer_format(values: list[str | None], *, exhaustive: bool = False) -> list[str]: ...
```

Minor issue since Python won't enforce it at runtime, but static type-checkers (pyright)
will accept invalid callers.

---

## 2. Documentation Issues

### 2.1 README format table is incomplete

The table shows a curated subset: ISO 8601 variants, space-separated, slash dates (US/EU
with 4-digit year only), compact, and Unix epochs. Entirely missing format families:

| Family | Example |
|---|---|
| Space datetime + timezone | `2024-01-15 10:30:00Z`, `2024-01-15 10:30:00+05:30` |
| 2-digit year slash dates | `01/15/24` (US), `15/01/24` (EU) |
| Dot-separated | `15.01.2024`, `15.01.24` |
| Month-name US | `Jan 15, 2024`, `Jan 15, 24` |
| Month-name EU | `15 Jan 2024`, `15 Jan 24` |
| 12-hour AM/PM | `2024-01-15 10:30:00 PM` |
| RFC 2822 | `Tue, 15 Jan 2024 10:30:00 +0000` |

**Fix:** add one representative row per missing family to the table, and update the
architecture section.

---

### 2.2 Architecture section has outdated `DateFmt` list and format count

The README says:

```
DateFmt:       Iso | SlashUS | SlashEU | Compact
TimeFmt:       Hms | HmsFrac | HmsCompact
… = 96 DateTime + 4 DateOnly + 4 Unix = 104 total
```

Actual `date.rs` has **13** `DateFmt` variants and `time.rs` has **5** `TimeFmt` variants.
The "104 total" count is significantly outdated.

**Fix:** update the architecture diagram and remove the specific count (it changes with each
new variant and is hard to keep accurate manually).

---

### 2.3 README missing usage examples for the primary convenience APIs

The README shows the low-level `infer_format()` + manual `str.to_datetime()` pattern, but
not the higher-level APIs that are the intended entry points:

- `infer_ts.to_datetime(series)` — one-liner series conversion
- `pl.col("ts").infer_ts.to_datetime()` — Polars expression plugin
- `pl.col("ts").infer_ts.infer_format()` — expression-level format inference
- `time_unit` and `raise_on_multiple` parameters

These are the APIs most users will reach for first.

**Fix:** add a "Quick start" section with these patterns before the existing detailed examples.

---

### 2.4 Installation section doesn't mention `uv`

The README says:

```sh
pip install maturin
maturin develop --release
```

But the project uses `uv` (lock file committed, `test-python.sh` calls `uv sync`). The
development workflow is actually:

```sh
uv sync --all-extras
maturin develop --release
```

The `uv`-based flow also pins exact dependency versions via `uv.lock`, which the `pip`
path doesn't.

---

### 2.5 Minor: `infer_format_expr` doc doesn't say it returns a scalar

`python/infer_ts/functions.py:60–81` — the docstring says "Returns a scalar String Series"
but the expression is applied at the Series level inside a `with_columns` call and returns
a length-1 Series broadcast across the dataframe. This is unusual and worth explaining
clearly with an example.

---

## 3. Code Quality / Maintenance

### 3.1 Format count in README will drift again

The architecture section enumerates component variants. Every time a new variant is added,
the README needs a manual update. Consider generating this documentation from the Rust code
(e.g., a test that prints/asserts the count) or at least removing the specific numbers.

### 3.2 `_EPOCH_UNITS` multiplier approach

`_EPOCH_UNITS` in `__init__.py` maps epoch markers to `(time_unit, multiplier)` pairs used
as: `(series.cast(Int64) * multiplier).cast(Datetime(unit))`. This is correct but brittle:

- Multipliers are implicit ("1000" means "seconds → ms")
- If a new Unix precision is added on the Rust side, the Python dict needs a manual update
- Comment should explain the semantics: "multiply source unit value to reach target unit"

The plugin's `scale_unix` function in `expressions.rs` uses exponents (`10^(target - from)`)
which is cleaner and self-documenting.

### 3.3 No test for `infer_format_expr` on ambiguous input

There is no test that verifies behavior of `.infer_ts.infer_format()` when multiple formats
match. Given the inconsistency noted in §1.2, this code path is untested.

### 3.4 No Rust `cargo test` in `test-python.sh`

`test-python.sh` only runs Python tests. The Rust unit tests (there are ~250+ of them in
`src/formats/mod.rs` and `src/inference.rs`) are not exercised by the project's test script.

**Fix:** add `cargo test` before `uv run pytest`.

### 3.5 `serde` is a non-optional production dependency

`Cargo.toml` lists `serde = { version = "1", features = ["derive"] }` as a main dependency
(not dev-only), because it's used by `expressions.rs` to deserialize Polars plugin kwargs.
This is correct and necessary — just noting it's not dead weight.

---

## 4. Missing Features (tracked, not urgent)

| Feature | Notes |
|---|---|
| Named timezone support | `EST`, `PST`, `JST` etc. — requires IANA tzdata integration |
| `time_unit` in series `to_datetime()` | See §1.3 — low effort to fix |
| CI/CD pipeline | v0.7 — GitHub Actions for manylinux wheels + PyPI publish |
| Property-based testing | `proptest` crate for fuzzy date/timezone validation |
| Changelog | No `CHANGELOG.md` — version history not tracked |

---

## 5. What's Working Well

- **CSP algorithm** — elegant, efficient, correct. Lazy domain init + early exit are well-designed.
- **Compositional format architecture** — adding a new date variant is genuinely easy (just
  a new `DateFmt` variant + `polars_date()` match arm).
- **Test coverage** — 250+ Rust unit tests + 160+ Python integration tests. Leap year
  validation, RFC 2822 day-of-week, case-insensitive parsing, null handling all covered.
- **PyO3 integration** — three dispatch paths (list, Series, iterator) are well-designed
  with appropriate zero-copy use.
- **Plugin performance** — ~1.1× native Polars casting overhead benchmarked and tested.
- **Python type stubs** — `__init__.pyi` and `_infer_ts.pyi` present; pyright configured.

---

## 6. Action Summary

| Priority | Item | Location |
|---|---|---|
| Bug | Use `env!("CARGO_PKG_VERSION")` | `src/lib.rs:116` |
| Bug | Add `raise_on_multiple` to `infer_format_expr` | `src/expressions.rs`, `python/infer_ts/functions.py` |
| Bug | Add `time_unit` to `to_datetime()` | `python/infer_ts/__init__.py` |
| Cleanup | Mark `exhaustive` keyword-only in `_infer_ts.pyi` | `python/infer_ts/_infer_ts.pyi` |
| Docs | Update README format table | `README.md` |
| Docs | Update README architecture section | `README.md` |
| Docs | Add quick-start section with convenience APIs | `README.md` |
| Docs | Add `uv` to installation instructions | `README.md` |
| Test | Add `cargo test` to `test-python.sh` | `test-python.sh` |
| Test | Add test for `infer_format_expr` on ambiguous input | `python/tests/` |
| Future | CI/CD pipeline (v0.7) | — |
| Future | Named timezone support | `src/formats/time.rs` |
