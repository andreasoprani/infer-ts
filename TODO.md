# TODO

---

## Future features

- [ ] Support also `to_date` and an argument to `to_datetime` to prefer a pl.Date dtype if the format is date-only
- [ ] Named timezone support (`EST`, `PST`, `JST` etc.) — requires IANA tzdata integration
- [ ] Property-based testing with `proptest` for fuzzy date/timezone validation
- [ ] Add a `CHANGELOG.md` to track version history

## Performance

- [ ] Investigate how to do single-pass infer and cast without huge differences in performance. We tried a naïve approach but it was 3x slower than native polars casting while if done properly the overhead should be negligible. Currently we are just doing a parsing pass and a casting pass piggybacking on polars' casting speed, but for exhaustive parsing or series entirely compatible with multiple formats this means parsing the whole series once and then casting it.
