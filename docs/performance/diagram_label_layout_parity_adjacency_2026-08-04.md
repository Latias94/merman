# Ordered Node Adjacency Performance Receipt

Status: preregistered; measurement pending

Revision 3 was preregistered on 2026-08-04 before inspecting any timing result from integrated
candidate `44eb98734fab9fef483ef0d5e845dd42218985f6`. Revision 1 named candidate `4feb70287924`, but a
source review rejected its per-node hash maps and historical tombstone scans before measurement.
Revision 2 named linked runtime `d3fd45e33`; every host-admission attempt failed before a benchmark
started, then remote `main` advanced and was merged. Only build-only compilation occurred for the
superseded candidates; neither produced a timing sample. Thresholds, surfaces, order, and sample
budgets remain unchanged.

## Question And Revisions

This receipt asks whether incrementally maintaining Graphlib-compatible ordered successor and
predecessor counts removes the rejected large-graph regression without materially regressing the
public reused-engine SVG render operation.

- Base: `1d571a007a00fe4027aee8f1e4c90dc8e42171b6`
- Runtime candidate: `d3fd45e33fafa0397c72d813ff3e76dc1a187a34`
- Candidate with benchmark harness: `0781a09dc893da2eb24576f3f0655cbe25895c82`
- Integrated candidate: `44eb98734fab9fef483ef0d5e845dd42218985f6`
- Base with the identical benchmark harness: `1187ea027f96dd95276ea826d2efb70bd88fc89c`
- Benchmark harness SHA-256: `65bc05e7c53cf940908b344a32e73b8596c9f02cfd9c1549acfe8e8a4be3488e`
- Toolchain: Rust `1.95.0` (`x86_64-pc-windows-msvc`)
- Public base worktree: `E:\Rust\merman-perf-label-base`
- Structural base worktree: `E:\Rust\merman-perf-label-base-bench`
- Candidate worktree: `E:\Rust\merman-perf-label-final`

Both worktrees must be clean detached checkouts. Builds are completed before host admission and no
warm-up, calibration, diagnostic, or prior-candidate sample is reused.

## Frozen Public Recipe

The public lane remains schema-v2 `render-svg`: native Criterion `pipeline`, reused process, reused
`Engine`, package `merman`, default features enabled, and explicit feature `svg`. It uses the same
four medium fixtures as the rejected receipt so the repaired Graphlib implementation is tested
through State, Class, Requirement, and Flowchart public rendering.

```powershell
python tools/bench/compare_self.py `
  --base-dir E:\Rust\merman-perf-label-base `
  --head-dir E:\Rust\merman-perf-label-final `
  --base-label 1d571a007-label-parity-base `
  --head-label 44eb98734-integrated-adjacency `
  --base-package merman `
  --head-package merman `
  --base-bench pipeline `
  --head-bench pipeline `
  --base-features svg `
  --head-features svg `
  --base-toolchain 1.95.0 `
  --head-toolchain 1.95.0 `
  --base-target-dir E:\Rust\merman\target\performance\label-parity-adjacency-base `
  --head-target-dir E:\Rust\merman\target\performance\label-parity-adjacency-final `
  --base-corpus tools/bench/corpus.json `
  --head-corpus tools/bench/corpus.json `
  --suite standard `
  --group end_to_end `
  --filter "end_to_end/(flowchart_medium|class_medium|state_medium|requirement_medium)" `
  --preset long `
  --evidence-mode confirmation `
  --calibration-pairs 16 `
  --max-pairs 64 `
  --start-side base `
  --relative-threshold-percent 10 `
  --absolute-threshold-us 50 `
  --confidence-level 0.95 `
  --bootstrap-seed 20260805 `
  --bootstrap-resamples 10000 `
  --timeout-seconds 600 `
  --out E:\Rust\merman\target\performance\diagram-label-layout-parity-adjacency.md `
  --json-out E:\Rust\merman\target\performance\diagram-label-layout-parity-adjacency.json
```

The larger calibration budget is fixed prospectively because the preceding receipt found the host
too noisy at eight pairs. Exit `0` is required for suite-wide conclusive non-regression. Exit `1`
is a confirmed regression, exit `2` is an evidence-contract failure, and exit `3` is inconclusive.
The regression boundary remains joint: more than 10 percent and more than 50 microseconds. No pair
budget will be extended after classification.

## Frozen Structural Surfaces

The independent variable remains node count `N = [50, 100, 200, 400, 800, 1600]`. Each graph has
ten nodes per layer and `14 * (N / 10 - 1)` directed edges. Four surfaces use the same committed
Criterion harness on both sides:

- `dugong_layout/plan`: full layout for `N = [50, 100, 200, 400, 800, 1600]`.
- `dugong_graph_build/plan`: graph construction for the same six plans, including adjacency state.
- `dugong_graph_build/parallel_edges_per_pair`: 64 unique endpoint pairs at `E/U = [1, 10, 100]`.
- `dugong_adjacency_churn/retain_last_then_query`: fanout `[64, 256, 1024, 4096]`, delete all but
  the last neighbor, then issue 256 first-successor queries.

Every surface runs in balanced `base A, candidate A, candidate B, base B` order. Each revision
shares one build target and saves every Criterion sample under a distinct immutable baseline name.

Build-only preparation:

```powershell
$env:CARGO_TARGET_DIR='E:\Rust\merman\target\performance\label-parity-adjacency-base-bench-dugong'
cargo +1.95.0 bench --locked -p dugong --bench layout --no-run

$env:CARGO_TARGET_DIR='E:\Rust\merman\target\performance\label-parity-adjacency-final-dugong'
cargo +1.95.0 bench --locked -p dugong --bench layout --no-run
```

For each of the four exact filters above, the measured sequence uses Criterion 0.8 defaults:

```powershell
$env:CARGO_TARGET_DIR='E:\Rust\merman\target\performance\label-parity-adjacency-base-bench-dugong'
cargo +1.95.0 bench --locked -p dugong --bench layout -- '<FILTER>' --save-baseline base-a

$env:CARGO_TARGET_DIR='E:\Rust\merman\target\performance\label-parity-adjacency-final-dugong'
cargo +1.95.0 bench --locked -p dugong --bench layout -- '<FILTER>' --save-baseline candidate-a
cargo +1.95.0 bench --locked -p dugong --bench layout -- '<FILTER>' --save-baseline candidate-b

$env:CARGO_TARGET_DIR='E:\Rust\merman\target\performance\label-parity-adjacency-base-bench-dugong'
cargo +1.95.0 bench --locked -p dugong --bench layout -- '<FILTER>' --save-baseline base-b
```

Before each measured command, ten one-second total-CPU samples must have median at most 15 percent
and maximum at most 30 percent. Failure delays the command and does not consume a run. Once a
measured command starts, its result is retained. The structural lane is inconclusive if, at any
input point, the two within-revision medians differ by more than 15 percent.

For each `N`, the reported candidate/base ratio is the geometric mean of
`candidate-a / base-a` and `candidate-b / base-b`; the absolute delta uses the corresponding
geometric-mean medians. Each run also fits ordinary least squares to `ln(median nanoseconds)` versus
`ln(N)`. The frozen acceptance rules are:

- Full layout: mean candidate slope increase at most `0.20`; neither `N=800` nor `N=1600`
  crosses the joint 10 percent and 50 microsecond regression threshold.
- Plan construction: mean candidate slope increase at most `0.20`; neither `N=800` nor `N=1600`
  crosses the joint 20 percent and 100 microsecond threshold.
- Parallel construction: candidate slope increase at most `0.10`; `E/U=100` does not cross the
  joint 25 percent and 100 microsecond threshold. This bounded allowance pays for the required
  parallel-edge count lifecycle that the base does not maintain.
- High-fanout churn: candidate slope increase at most `0.15`; fanout 4096 does not cross the joint
  25 percent and 100 microsecond threshold.

The representation also has a source-checkable allocation topology: two graph-wide ordered-list
vectors scale with node slots, while one graph-wide endpoint map, link vector, and reusable free
list scale with peak unique endpoint pairs. There is no heap-owning container per node, and queries
follow only live links rather than historical slots. The deterministic 4096-operation ordered-count
oracle and the high-fanout remove/reuse test are mandatory evidence for those invariants.
Correctness and stable-identity gates remain mandatory regardless of timing.

## Frozen Correctness Gates

- `cargo nextest run -p dugong-graphlib -p dugong --no-fail-fast`
- `cargo nextest run -p merman-render --no-fail-fast`
- The signed State and Class semantic-label and SVG canaries remain green, together with the
  focused Dugong ordering and geometry tests derived from the historical same-input audit.
- The all-family SVG and semantic-label comparison remains green.

## Results

Pending. This section may record observations and classification, but the revisions, fixtures,
commands, order, admission rule, pair budget, thresholds, seed, resample count, and curve rules
above must not change.
