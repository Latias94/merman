# Empty source-config owner — macOS, October 19, 2026

The private operation configuration builder now shares its existing empty source configuration
instead of constructing a second empty owner. Default Packet, Kanban and Quadrant each save four
allocations and 240 allocated bytes. All 418 theme-catalog cells improve by the same count and
bytes. This is allocation/work evidence; no latency or v0.9.0 convergence claim follows.

The clean baseline is `6cfeb8ef7`, on Apple M4 Pro, 48 GiB, macOS aarch64, Rust 1.95.0. Both sides
use release opt3 and all public features. The ignored preregistration, frozen probes, patch, input
hashes and raw results are in `target/bench/experiments/empty-source-config-owner-20261019/`.
All 632 successful controls, 26 public errors and 418 catalog cells are inherited from the accepted
Treemap checkpoint. Three independently required primaries are Packet, Kanban and Quadrant:
allocation count and allocated bytes must both strictly decrease for each. All four memory
metrics must be nonincreasing on every jointly stable control and stable catalog cell. Cargo,
allocation probes and checks run serially.

## Change and safety boundary

Only `OperationConfigBuilder::new` changes: the empty-overrides branch clones the existing
`MermaidConfig` handle. Nonempty filtering/merging is untouched. The removed construction creates
three Arc owners: JSON, explicit paths and mutation paths. Destroying that extra unique JSON owner
also creates the nonrecursive drop stack. The measured saving is therefore four allocations,
rather than the initial three-owner source estimate. The retained original source configuration
still follows its existing lifetime and destruction path.

An empty JSON object can carry mutation or compatibility metadata. Safety does not depend on
assuming that metadata is absent. This private `source` layer's three consumers only read JSON:
appearance selection, theme materialization and paint ownership's path lookup. They do not consume
its compatibility, explicit-path or mutation metadata, and do not store the source handle in a
public result. `ParseMetadata.config` still receives the original preprocessed config. Effective
configuration, detector checkpoints/epoch, mutation replay, appearance ordering, secure filtering,
paint freezing, cancellation and error checkpoints are unchanged. The shared source is read-only;
ordinary copy-on-write preserves isolation. If a future source consumer starts reading metadata,
this value-only boundary needs review again.

v0.9.0 already clones overrides in the corresponding empty branch. `fdf302d31`, which centralizes
operation theme ownership, introduces the extra empty construction. Other suspected full-config
copies were excluded: defaults already share Arc storage, unchanged detector snapshots skip JSON
comparison, no-theme paint preparation exits early, and compatibility projections were either
present in the tag or absent from ordinary deterministic SVG. This small fixed saving does not
explain the complete Packet/Kanban release gap.

## Complete public measurements

Each row measures a complete public render and retains its rich result at the memory checkpoint.
Input loading, theme compilation and result serialization stay outside the operation. Counts
include allocation/reallocation observations, with one warmup and five samples per control.

| Control | Allocations before → after | Allocated bytes before → after | Peak growth | Retained growth |
| --- | ---: | ---: | ---: | ---: |
| Packet medium | 117 → 113 | 19,162 → 18,922 | 10,148 | 3,855 |
| Kanban medium | 620 → 616 | 115,694 → 115,454 | 45,838 | 30,319 |
| Quadrant medium | 400 → 396 | 38,159 → 37,919 | 19,163 | 7,599 |
| Sequence medium | 4,572 → 4,568 | 744,888 → 744,648 | 287,877 | 60,271 |
| Treemap medium | 750 → 746 | 92,488 → 92,248 | 33,658 | 18,807 |

All 632 successful controls preserve SVG/report identities and ordered custom-measurer traces;
all 26 errors match exactly. Of 491 jointly stable rows, 459 improve and 32 remain unchanged,
with no increase in any memory metric. Every improvement saves four allocations and 240 bytes;
457 retain their peak and two reduce it by 168 bytes. Retained growth never changes. Controls
with nonempty authored source overrides retain their original path and account for the unchanged
rows. The 141 variable rows remain inconclusive without retry; the prior variable list only loses
`family-flowchart_direction_conflict`, which is now stable on both sides.

All 418 catalog cells retain exact warmup result metadata and all five saved sample SVG/report
identities, together with configuration, diagnostics, theme status and family identity.
All memory rows are stable and all save four allocations and
240 bytes. Sixty-six lower peak growth by 168 bytes and 352 retain it; retained growth is unchanged
everywhere. These cells cover 35 logical families, ten presets and no theme. Catalog and control
totals stay within their original request/result recipes.

## Validation and next work

Independent source review finds no blocking issues. Existing copy-on-write isolation, historical
mutation journals, nested detection, thread epochs and cancellation tests cover the changed
boundary; no pointer-equality or clone-mirroring test is added. The focused core release nextest
run passes 187 tests. The expanded core/public/renderer release nextest run passes 7,645 tests
with seven skipped. Release all-feature Clippy for those three packages and the minimal-feature
core check pass. All 37 DOM entries pass structure, parity and parity-root modes with existing
browser-text-layout receipts. The full optimized WASM package builds, including wasm-opt; WASM
runtime smoke was not repeated. Formatting, diff and changed-path checks pass. Independent raw
review verifies the frozen inputs, linked probes, four allocation sources and report numbers.

The all-family goal stays active. A separate benchmark-method correction must align non-regression
classification with its documented joint boundary, retaining old receipts and fresh calibration
for later release observations. Copied terminal receipt values remain an independent candidate.
