# Diagram Label Layout Parity Performance Receipt

Status: preregistered; measurement pending

Preregistered on 2026-08-04 before inspecting any base/head timing result.

## Question And Revisions

This receipt asks whether the source-backed label, State/Class graph, Dugong, and Graphlib repairs
introduce a material regression in the public reused-engine SVG render operation, and whether the
full Dugong layout curve develops worse scale growth.

- Base: `1d571a007a00fe4027aee8f1e4c90dc8e42171b6`
- Candidate: `599c2195b7fb84164397e921325bf8deb7f117fa`
- Toolchain: Rust `1.95.0` (`x86_64-pc-windows-msvc`)
- Base worktree: `E:\Rust\merman-perf-label-base`
- Candidate worktree: `E:\Rust\merman-perf-label-head`
- Base target: `E:\Rust\merman\target\performance\label-parity-base`
- Candidate target: `E:\Rust\merman\target\performance\label-parity-head`

Both worktrees must be clean detached checkouts. The runner must independently verify locked
builds, executable digests, corpus manifests, and byte-identical selected fixtures.

## Frozen Public Recipe

The public lane is schema-v2 `render-svg`: native Criterion `pipeline`, reused process, reused
`Engine`, package `merman`, default features enabled, and explicit feature `svg`. The selected
fixtures are `flowchart_medium`, `class_medium`, `state_medium`, and `requirement_medium` from the
`standard` suite.

```powershell
python tools/bench/compare_self.py `
  --base-dir E:\Rust\merman-perf-label-base `
  --head-dir E:\Rust\merman-perf-label-head `
  --base-label 1d571a007-label-parity-base `
  --head-label 599c2195b-label-parity-head `
  --base-package merman `
  --head-package merman `
  --base-bench pipeline `
  --head-bench pipeline `
  --base-features svg `
  --head-features svg `
  --base-toolchain 1.95.0 `
  --head-toolchain 1.95.0 `
  --base-target-dir E:\Rust\merman\target\performance\label-parity-base `
  --head-target-dir E:\Rust\merman\target\performance\label-parity-head `
  --base-corpus tools/bench/corpus.json `
  --head-corpus tools/bench/corpus.json `
  --suite standard `
  --group end_to_end `
  --filter "end_to_end/(flowchart_medium|class_medium|state_medium|requirement_medium)" `
  --preset long `
  --evidence-mode confirmation `
  --calibration-pairs 8 `
  --max-pairs 32 `
  --start-side base `
  --relative-threshold-percent 10 `
  --absolute-threshold-us 50 `
  --confidence-level 0.95 `
  --bootstrap-seed 20260804 `
  --bootstrap-resamples 10000 `
  --timeout-seconds 600 `
  --out E:\Rust\merman\target\performance\diagram-label-layout-parity.md `
  --json-out E:\Rust\merman\target\performance\diagram-label-layout-parity.json
```

The recipe is fixed at eight balanced base A/A pairs, eight balanced candidate A/A pairs, and the
power-derived even fresh AB/BA count capped at 32. No diagnostic or calibration observation may be
reused, no fixture may be removed, and the budget will not be extended after classification.

Exit `0` is required for suite-wide conclusive non-regression. Exit `1` is a confirmed regression,
exit `2` is an evidence-contract failure, and exit `3` is inconclusive. The ordinary regression
boundary is joint: more than 10 percent and more than 50 microseconds.

## Frozen Structural Curve

The independent variable is node count `N = [50, 100, 200, 400, 800, 1600]`. Each graph has ten
nodes per layer and `14 * (N / 10 - 1)` directed edges. Both revisions run the existing full public
Dugong layout benchmark with identical default Criterion sampling:

```powershell
$env:CARGO_TARGET_DIR='E:\Rust\merman\target\performance\label-parity-base-dugong'
cargo +1.95.0 bench --locked -p dugong --bench layout -- 'dugong_layout/plan'

$env:CARGO_TARGET_DIR='E:\Rust\merman\target\performance\label-parity-head-dugong'
cargo +1.95.0 bench --locked -p dugong --bench layout -- 'dugong_layout/plan'
```

For each `N`, the receipt will read Criterion's median point estimate and report candidate/base.
It will also fit ordinary least squares to `ln(median nanoseconds)` versus `ln(N)` across all six
points. The structural curve passes only when the candidate slope exceeds the base slope by no more
than `0.20` and neither `N=800` nor `N=1600` regresses by both more than 10 percent and more than 50
microseconds. This is a scale non-regression check, not a speedup claim. Correctness and stable
identity gates remain mandatory regardless of timing.

## Results

Pending. This section may record observations and classification, but the revisions, fixtures,
commands, pair budget, thresholds, seed, resample count, and curve rules above must not change.
