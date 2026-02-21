# TODO

---

## Distribution

- [ ] Add `cargo test` to `test-python.sh` so Rust unit tests run alongside Python tests
- [ ] CI pipeline: build manylinux wheels with maturin and publish to PyPI

## Docs

- [ ] Auto-generate the supported format tables in a separate `FORMATS.md` from the Rust source (e.g. a `cargo test -- --ignored dump_formats` that writes the file), then reference it from the README instead of maintaining the tables by hand.

## Bug fixes / API consistency

- [x] Use `env!("CARGO_PKG_VERSION")` in `src/lib.rs` instead of hardcoded `"0.1.0"` to keep `__version__` in sync with `Cargo.toml`
- [x] Add `time_unit` parameter to `infer_ts.to_datetime()` (series-level API) — currently only the Polars plugin exposes this
- [x] Mark `exhaustive` as keyword-only (`*` separator) in `python/infer_ts/_infer_ts.pyi` stubs

## Performance

- [ ] Investigate how to do single-pass infer and cast without huge differences in performance. We tried a naïve approach but it was 3x slower than native polars casting while if done properly the overhead should be negligible. Currently we are just doing a parsing pass and a casting pass piggybacking on polars' casting speed, but for exhaustive parsing or series entirely compatible with multiple formats this means parsing the whole series once and then casting it.
