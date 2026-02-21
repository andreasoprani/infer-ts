# TODO

Items are grouped by release milestone. Work through them roughly top-to-bottom;
later sections depend on earlier ones being solid.

---

## v0.6 – Polars plugin (single-pass infer + cast)

- [x] Expose as a Polars expression plugin: `df.with_columns(pl.col("ts").infer_ts.to_datetime())`, simple setup with two passes, infer and then cast.
- [x] **Single-pass datetime parsing**
  - Infer format and cast to datetime in one pass over the data
  - `InferState` feeds values directly; early-exit path buffers only pre-settled values
  - `Format::parse_to_us()` converts strings to µs without a second Arrow iteration
- [x] **Format hint parameter**
  - Accept an optional format hint to skip inference when format is known
  - Useful for performance when format is predetermined
- [x] ~~**Architecture considerations**~~ (done in v0.2)
  - ~~Current `Vec<Format>` return type prepares for trying formats in priority order~~
  - ~~Consider returning `InferResult` struct with: formats, confidence score, error details~~
  - Compositional format architecture now in place; extending formats is straightforward
- [x] Speed up execution to try to match native polars casting performance
- [x] Allow the user to define the datetime time_unit they want (ns, us or ms)
- [x] Remove `format` from to_datetime kwargs, it's pretty useless, if one already knows the format they can use native casting.

## v0.7 - Distribution

- [ ] CI pipeline: build manylinux wheels with maturin and publish to PyPI
- [ ] Add `cargo test` to `test-python.sh` so Rust unit tests run alongside Python tests

## v0.8 - Bug fixes / API consistency

- [ ] Use `env!("CARGO_PKG_VERSION")` in `src/lib.rs` instead of hardcoded `"0.1.0"` to keep `__version__` in sync with `Cargo.toml`
- [ ] Add `raise_on_multiple` kwarg to `infer_format_expr` (Rust + Python), consistent with `to_datetime_expr` — currently it silently returns the first format when multiple match
- [ ] Add `time_unit` parameter to `infer_ts.to_datetime()` (series-level API) — currently only the Polars plugin exposes this
- [ ] Mark `exhaustive` as keyword-only (`*` separator) in `python/infer_ts/_infer_ts.pyi` stubs

## v0.9 - Performance

- [ ] Investigate how to do single-pass infer and cast without huge differences in performance. We tried a naïve approach but it was 3x slower than native polars casting while if done properly the overhead should be negligible. Currently we are just doing a parsing pass and a casting pass piggybacking on polars' casting speed, but for exhaustive parsing or series entirely compatible with multiple formats this means parsing the whole series once and then casting it.
