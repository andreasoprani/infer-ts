# Plan: Accept Polars Series directly via PyO3 (zero-copy)

## Context

Currently `to_datetime()` calls `series.to_list()` in Python (O(n) Python object creation), then passes that list to Rust where PyO3 converts it to `Vec<Option<String>>` (O(n) Rust String allocations). For a 1M-row column where the format is unambiguous from the first value, this wastes ~2M allocations.

The fix: use `pyo3-polars` to accept a Polars `Series` directly in Rust, access the underlying Arrow `StringChunked` for zero-copy iteration over `Option<&str>` values. Combined with early-exit in the inference engine, this means we touch only as many values as needed.

Reference: [cookiecutter-polars-plugins](https://github.com/MarcoGorelli/cookiecutter-polars-plugins) shows pyo3-polars + abi3 works together.

## Files to Modify

- `Cargo.toml` — upgrade pyo3 0.22→0.26, add pyo3-polars + polars
- `pyproject.toml` — bump minimum Python to 3.9
- `src/inference.rs` — generalize `infer()` to accept iterators
- `src/lib.rs` — add `infer_format_series()`, update module registration, handle pyo3 0.26 API changes
- `python/infer_ts/__init__.py` — use series path in `to_datetime()`, dispatch in `infer_format()`
- `python/infer_ts/_infer_ts.pyi` — add stub for `infer_format_series`
- `python/infer_ts/__init__.pyi` — update `infer_format` to accept Series too

## Implementation Steps

### Step 1: Update `Cargo.toml`

```toml
[dependencies]
pyo3 = { version = "0.26", features = ["macros", "abi3", "abi3-py39"] }
pyo3-polars = { version = "0.25", default-features = false }
polars = { version = "0.52", default-features = false }
chrono = "0.4"
```

Key changes:
- pyo3 0.22 → 0.26 (required by pyo3-polars 0.25)
- abi3-py38 → abi3-py39 (pyo3 0.26 minimum)
- polars with default-features=false keeps it lean (just core Series/ChunkedArray)

### Step 2: Update `pyproject.toml`

- `requires-python = ">=3.9"` (matches abi3-py39)

### Step 3: Fix pyo3 0.26 API changes in `src/lib.rs`

pyo3 0.26 changed `#[pymodule]` signature — check and adapt.

### Step 4: Generalize `infer()` in `src/inference.rs`

Change signature to accept any iterator of `Option<&str>`:

```rust
pub fn infer<'a>(values: impl IntoIterator<Item = Option<&'a str>>, exhaustive: bool) -> Vec<Format>
```

The loop body stays identical. Update call sites to pass owned iterators.

### Step 5: Add `infer_format_series()` in `src/lib.rs`

```rust
use pyo3_polars::PySeries;

#[pyfunction]
#[pyo3(signature = (series, exhaustive=false))]
fn infer_format_series(series: PySeries, exhaustive: bool) -> PyResult<Vec<String>> {
    let series = series.0;
    let ca = series.str().map_err(|e| PyErr::new::<pyo3::exceptions::PyTypeError, _>(e.to_string()))?;
    let formats = inference::infer(ca.into_iter(), exhaustive);
    Ok(formats.iter().map(|f| f.polars_format()).collect())
}
```

`StringChunked::into_iter()` yields `Option<&str>` — exactly what `infer()` needs. Zero copy.

### Step 6: Update Python wrapper `__init__.py`

Make `infer_format` accept both list and Series. Update `to_datetime()` to use the series path directly.

### Step 7: Update type stubs

Add `infer_format_series` stub and update `infer_format` to accept `list | Series`.

## Verification

1. `cargo test` — all Rust tests pass
2. `cargo clippy` — no warnings
3. `cargo fmt -- --check` — formatted
4. `bash test-python.sh` — all existing Python tests pass
5. Add Python test passing `pl.Series` directly to `infer_format()`
