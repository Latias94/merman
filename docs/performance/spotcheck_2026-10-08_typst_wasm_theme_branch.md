# Typst WASM size spotcheck — 2026-10-08

The Typst owner job measured the current theme branch with the pinned `typst-wasm` recipe:

```text
cargo run --locked -p xtask -- wasm-size-matrix \
  --surface typst \
  --budget-file docs/release/WASM_SIZE_BUDGETS.json
```

The measurement came from commit `18a0e677b` before the CI-only feature/test corrections. The
renderer and theme sources were unchanged for this artifact measurement.

| Artifact | Raw | Stripped | Gzip | Brotli |
| --- | ---: | ---: | ---: | ---: |
| `typst-wasm` | 19,522,093 | 11,870,557 | 4,566,114 | 3,390,895 |

The raw and Brotli ceilings were raised to 19,718,000 and 3,425,000 bytes respectively. The
stripped and gzip ceilings already contained sufficient headroom. This records the cost of the
typed theme and owned-rendering graph in the Typst surface; it does not claim that the increase is
free, only that the measured artifact is now represented by the owner budget.
