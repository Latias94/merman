# Sequence empty CSS groups — macOS, October 19, 2026

Sequence no longer constructs groups for empty text declarations that its final filter always
discards. The default public medium fixture saves ten allocations and 788 allocated bytes per
complete render. Peak and retained growth remain unchanged. This admits redundant-work removal;
it does not establish latency improvement or convergence with v0.9.0.

## Experiment and semantics

The clean baseline is `e467a28e5fe50035f130eafe6e7405d53838dbe1`, on Apple M4 Pro, 48 GiB,
macOS aarch64, Rust 1.95.0. Both sides use release opt3 and all public features. The ignored ledger,
frozen patch, executables, input hashes and raw results are under
`target/bench/experiments/sequence-empty-css-groups-20261019/`.

The question, frozen baseline, complete inherited corpus, strict allocation-count/byte gate and
validation scope were registered before editing. The preceding Sequence checkpoint's frozen
executables and results supply the baseline; its production source equals this experiment's base.
Intervening documentation and Rustdoc receipt changes do not change the compiled production source.
The candidate library and probes were rebuilt and recorded separately. Cargo and probes ran serially.

Previously each of four role buckets built a temporary group and surface vector even when all
declarations were empty. Only after grouping did a filter discard empty groups. The new branch
checks the declaration after its original getter call and skips the discarded group immediately.
It removes the final filter; nonempty grouping and surface order remain unchanged. There is no new
cache, wrapper, validation shortcut or theme-presence gate. Empty and nonempty declarations can
interleave without changing the retained projection. Existing text, fill, baseline, effect,
receipt, budget and cancellation paths remain intact.

The discarded grouping was introduced in `b997dc160` during immutable theme binding preparation.
v0.9.0's Sequence stylesheet writer has no corresponding typed text-group preparation. This is a
specific theme-refactor overhead; it does not explain the entire release rendering difference.

## Exact public measurements

Each public render uses deterministic measurement and keeps its rich result alive at the memory
checkpoint. Theme compilation, input loading and result serialization are outside the operation.
Allocation counts include the allocator's allocation/reallocation observations. Each row has one
warmup and five samples; variable rows remain inconclusive without retry.

| Control | Allocations before → after | Allocated bytes before → after | Peak growth | Retained growth |
| --- | ---: | ---: | ---: | ---: |
| Sequence medium | 4,598 → 4,588 | 746,226 → 745,438 | 288,415 | 60,271 |
| Sequence tiny | 2,348 → 2,338 | 511,092 → 510,304 | 247,786 | 40,493 |
| Rich classic, no theme | 3,420 → 3,410 | 586,574 → 585,786 | 255,113 | 39,025 |
| 64 messages, no theme | 8,583 → 8,573 | 1,144,821 → 1,144,033 | 359,525 | 80,479 |

All 488 successful controls retain exact SVG, report and ordered custom-measurer trace. Of 347
jointly stable memory rows, 34 improve by exactly ten allocations and 788 bytes, while 313 remain
unchanged. Every improved row is a no-theme Sequence request, including three looks, raw
configuration/fallback controls and 0/1/16/64-message cases. Peak and retained growth never change.
The 141 variable rows retain the preceding experiment's exact names: 134 State and seven Flowchart
controls. All 26 inherited public errors match exactly.

The catalog covers 38 scenes, 35 logical families and ten presets plus no theme: 418 cells. All
warmups, five samples, SVG/report identities, configuration, diagnostics, theme status and family
identity match. All memory rows are stable. Only Sequence's no-theme cell improves, by ten
allocations and 788 bytes; the other 417 cells, including all ten typed Sequence presets, are
unchanged. Expanded typed recipes already have nonempty declarations. Catalog and site-control
byte totals differ because their request and retained-result recipes differ; comparisons stay
within their original lanes.

Independent source and raw-evidence reviews found no blocking issues. Existing actor/loop local
ownership and seven-surface tests cover empty/nonempty interleaving, grouping and repeated output;
no implementation-mirroring test was added.

## Validation and next work

The focused release nextest run passes 326 tests. The full renderer release run passes 5,207 tests
with five skipped, and release all-feature Clippy passes. The full 37-entry DOM comparison passes
structure, parity and parity-root modes with the existing browser-text-layout receipts. The full
optimized WASM package builds, including wasm-opt; WASM runtime smoke was not repeated. Formatting,
diff and changed-path checks pass.

The all-family v0.9.0 goal remains active. The next independent allocation experiment should borrow
the shared Info/Error/Quadrant/XY CSS success receipt from its existing prepared font owner,
preserving all three recorded values and successful-write ordering. Treemap has a separate borrowed
receipt candidate. Neither source analysis nor the measurements above establish a clock result.
