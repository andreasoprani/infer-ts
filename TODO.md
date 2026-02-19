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
- [ ] Speed up execution to try to match native polars casting performance
- [ ] Allow the user to define the datetime time_unit they want (ns, us or ms)
- [ ] Remove `format` from to_datetime kwargs, it's pretty useless, if one already knows the format they can use native casting.

## v0.7 - Distribution

- [ ] CI pipeline: build manylinux wheels with maturin and publish to PyPI
