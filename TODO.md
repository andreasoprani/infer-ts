# TODO

---

## Distribution

- [x] Add `cargo test` to `test-python.sh` so Rust unit tests run alongside Python tests — renamed to `build-and-test.sh`
- [x] CI pipeline: build and run tests — `.github/workflows/ci.yml`
- [ ] CI pipeline: build manylinux wheels with maturin and publish to PyPI

## Docs

- [ ] Auto-generate the supported format tables in a separate `FORMATS.md` from the Rust source (e.g. a `cargo test -- --ignored dump_formats` that writes the file), then reference it from the README instead of maintaining the tables by hand.

## Future features

- [ ] Named timezone support (`EST`, `PST`, `JST` etc.) — requires IANA tzdata integration
- [ ] Property-based testing with `proptest` for fuzzy date/timezone validation
- [ ] Add a `CHANGELOG.md` to track version history

## Performance

- [ ] Investigate how to do single-pass infer and cast without huge differences in performance. We tried a naïve approach but it was 3x slower than native polars casting while if done properly the overhead should be negligible. Currently we are just doing a parsing pass and a casting pass piggybacking on polars' casting speed, but for exhaustive parsing or series entirely compatible with multiple formats this means parsing the whole series once and then casting it.
