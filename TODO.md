# TODO

---

## Blockers (PyPI publishing)

- [x] Add a `LICENSE` file to the repo root (`pyproject.toml` has `license = { text = "MIT" }` inline but no actual file; `Cargo.toml` has no license field at all)
- [x] Add `[project.dependencies]` with `polars` — it's imported at module load time but currently only listed under `[project.optional-dependencies].dev`, so `pip install infer-ts` would break immediately

## Critical bugs

- [x] Integer overflow in `scale_unix` (`src/expressions.rs:155-157`) — upscaling Unix seconds/ms/μs to nanoseconds multiplies `i64` without overflow checks; Rust release builds wrap silently, same issue in `python/infer_ts/__init__.py:104` via Polars `Int64` arithmetic
- [x] README epoch example is wrong (`README.md:190-193`) — shows `cast(pl.Int64).cast(pl.Datetime("us"))` for Unix seconds, which interprets raw seconds as microseconds, producing wildly wrong dates; correct approach requires scaling first (as `to_datetime()` does correctly)

## Bugs / correctness

- [x] Wrong RFC 2822 day-of-week in example (`src/formats/date.rs:464`, `FORMATS.md:24`) — `"Tue, 15 Jan 2024"` but January 15 2024 was a Monday; the parser validates weekday so this example would be rejected by its own parser; fix to `"Mon, 15 Jan 2024"` in `date.rs` (FORMATS.md is auto-generated)
- [x] Stale false docstring in `__init__.py:53` — claims "Lists use a faster bulk path" but lists go through the same `iter()` path as everything else; remove the claim
- [x] `to_datetime()` docstring says "Returns: A Polars Series with Datetime (or Date) dtype" (`__init__.py:79`) — Date is never returned, always Datetime

## API / type annotation inconsistencies

- [x] `time_unit: str` instead of `Literal["ns", "us", "ms"]` in `functions.py:27` and `namespace.py:26` — should match `__init__.py` and the stub
- [x] `ExprInferTsNamespace` not exported from `__init__.pyi` — users can't type-annotate against it
- [x] `iter` parameter name in `_infer_ts.pyi:16` shadows the Python builtin `iter()` — rename to `values` or `iterable`

## Docs / metadata

- [x] Add missing `pyproject.toml` metadata: `[project.urls]` (homepage, repository, changelog), `authors`, `readme = "README.md"`, `keywords`, per-version Python classifiers (`3.9`–`3.13`)
- [x] Consolidate split dev dependencies — `pyproject.toml` has both `[project.optional-dependencies].dev` and `[dependency-groups].dev` with overlapping/inconsistent contents (`pyarrow` in both, `polars`/`pytest` missing from one)
- [x] Update README examples to pass Series directly instead of `.to_list()` (`README.md:164,187`)
- [x] Add `license` field to `Cargo.toml` (e.g. `license = "MIT"`)

## Tests

- [ ] Fix test assertions that hedge with `pl.Date or pl.Datetime` — since `to_datetime()` always returns `Datetime`, assert that specifically (multiple places in `test_polars_roundtrip.py`)
- [ ] Add Python-level test for `exhaustive=True` returning multiple formats on ambiguous data
- [ ] Add roundtrip test for RFC 2822 date-only format (`"Mon, 15 Jan 2024"`)
- [ ] Add roundtrip tests for spaced-timezone formats (`"2024-01-15 10:30:00 +05:30"`) via Python API
- [ ] Add plugin-path tests for dot-separated EU, month-name, and RFC 2822 formats in `test_plugin.py`
- [x] Add test for `scale_unix` overflow boundary (e.g. `9_999_999_999` seconds with `time_unit="ns"`)
- [ ] Move shared `_dt`/`_d` helpers from `test_plugin.py:14` and `test_polars_roundtrip.py:21` into a `conftest.py`
- [ ] Mark fragile timing assertions in `test_benchmark.py:50,230` with `pytest.mark.slow` and exclude from normal CI

## Code quality

- [ ] `FeedResult` and `InferResult` in `src/inference.rs:30,40` are `pub` but effectively crate-private — change to `pub(crate)`
- [ ] Add `codegen-units = 1` to `[profile.release]` in `Cargo.toml` — pairs with the existing `lto = true` for full LTO benefit
- [x] Fix trailing double blank line at end of `python/infer_ts/functions.py`

## Distribution

- [ ] pre-commit hook for ruff formatting of the python side
- [ ] CI step for pyright and ruff formatting of the python side
- [ ] CI pipeline: build manylinux wheels with maturin and publish to PyPI


## Future features

- [ ] Support also `to_date` and an argument to `to_datetime` to prefer a pl.Date dtype if the format is date-only
- [ ] Named timezone support (`EST`, `PST`, `JST` etc.) — requires IANA tzdata integration
- [ ] Property-based testing with `proptest` for fuzzy date/timezone validation
- [ ] Add a `CHANGELOG.md` to track version history

## Performance

- [ ] Investigate how to do single-pass infer and cast without huge differences in performance. We tried a naïve approach but it was 3x slower than native polars casting while if done properly the overhead should be negligible. Currently we are just doing a parsing pass and a casting pass piggybacking on polars' casting speed, but for exhaustive parsing or series entirely compatible with multiple formats this means parsing the whole series once and then casting it.
