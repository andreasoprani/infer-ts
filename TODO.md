# TODO

---

## API

- [ ] Consider dropping the `pl.col("ts").infer_ts.to_datetime()` expression namespace in favour of the unified `infer_ts.to_datetime(col)` API. The namespace requires manual type-stub configuration (`stubPath`) because `@pl.api.register_expr_namespace` is runtime-only and pyright cannot discover it statically. The function form is equally concise and works out of the box. The main loss would be mid-expression chaining, which is minor. Blocked on: deciding whether polars-ecosystem convention is worth the friction.

## Distribution / type checking

- [ ] Expose bundled Polars expr stubs to end users. The wheel already ships `infer_ts/_stubs/polars/expr/expr.pyi` (overrides `Expr.infer_ts`), but there is no `get_stubs_path()` function and no README documentation pointing to it. Users who install from PyPI and use pyright will still hit "Cannot access attribute 'infer_ts' for class 'Expr'". Fix: re-add `get_stubs_path() -> Path` to the public API and document the one-line `pyproject.toml`/`pyrightconfig.json` configuration needed.

## Future features

- [ ] Support also `to_date` and an argument to `to_datetime` to prefer a pl.Date dtype if the format is date-only
- [ ] Named timezone support (`EST`, `PST`, `JST` etc.) — requires IANA tzdata integration
- [ ] Property-based testing with `proptest` for fuzzy date/timezone validation
- [ ] Add a `CHANGELOG.md` to track version history

## Performance

- [ ] Investigate how to do single-pass infer and cast without huge differences in performance. We tried a naïve approach but it was 3x slower than native polars casting while if done properly the overhead should be negligible. Currently conversion uses early-exit inference and a casting pass piggybacking on polars' casting speed. For series entirely compatible with multiple formats, inference scans the whole series before casting it.
