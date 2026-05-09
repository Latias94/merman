# Full Benchmark Gate

This report records a release-gate benchmark run for the current fearless-refactor state.

Command:

```bash
cargo bench -p merman --features render
```

## Result

- Status: passed.
- Wall time: approximately `52m 8s`.
- Scope: package-level `merman` benches with `render`, including the pipeline bench suite,
  render/layout stress benches, and `text_measure_stress`.

## Notes

- An earlier 20-minute attempt timed out, so the successful run used a wider execution window.
- Criterion reported mixed local change classifications against saved local baselines. Treat this
  run as release-gate evidence that the benchmark suite executes successfully, not as a targeted
  performance attribution report.
- More focused cross-repo and stage-attribution evidence remains in `COMPARISON.md` and the
  standard canary spotcheck reports.
