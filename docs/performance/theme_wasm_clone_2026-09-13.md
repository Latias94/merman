# Theme evidence clone size experiment — 2026-09-13

The out-of-line clone experiment is **rejected**. It does not reduce the final optimized,
stripped Typst plugin. Retain the derived `Clone` implementation of `FamilyThemeEvidence`.

Source baseline: `317acd7db`, macOS ARM64, Rust 1.95.0, the existing `typst-wasm` publish recipe.
The candidate copied the same seven fields but applied `inline(never)` to the clone only on
WASM, retaining the derived implementation's inline behavior on native targets. Neither
capabilities, dependencies, compiler flags, resource policies, nor size budgets changed.

| Metric, bytes | Baseline | Candidate | Difference |
| --- | ---: | ---: | ---: |
| Raw | 18,619,942 | 18,619,942 | 0 |
| Optimized and stripped | 11,535,833 | 11,535,833 | 0 |
| gzip | 4,426,399 | 4,426,371 | -28 |
| Brotli | 3,267,940 | 3,266,411 | -1,529 |

One build/measurement ran per variant using the same command, sequentially with
`CARGO_BUILD_JOBS=2`:

```text
cargo run --locked -p xtask -- wasm-size-matrix --surface typst --budget-file docs/release/WASM_SIZE_BUDGETS.json
```

The frozen primary threshold was at least 16 KiB less optimized/stripped code, with no growth in
any other size metric. Compression-only differences do not pass it. Both variants exceed all four
existing Typst budgets. The compiler attribution also leaves `merge_theme_evidence` at 58,822
shallow bytes; a large caller alone is not evidence that these collection clones were inlined.
The raw module's function-name subsection is stripped from the delivered artifact and is not a
source of final-package savings.

The candidate passed 196 family tests before measurement. It was then removed; broader candidate
admission gates were unnecessary after size rejection. No latency or memory improvement is
claimed. The experiment ledger, raw outputs, baseline artifact, and attribution reports remain in
`target/bench/experiments/typst-evidence-clone-20260913/`.
