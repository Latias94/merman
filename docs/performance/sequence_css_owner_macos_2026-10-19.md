# Sequence CSS owner checkpoint — macOS, October 19, 2026

Sequence's stylesheet writer now borrows the final scalar values already owned by its immutable
prepared artifact. This removes duplicate default and typed font, shape paint and activation
Strings, and the extra shared compatibility reference. The default public medium fixture saves
fourteen allocations and 497 allocated bytes. This checkpoint admits allocation/work removal;
it does not establish a latency improvement or convergence with v0.9.0.

## Experiment and ownership

The clean baseline is `17e9b121e87d3194dd9f1a192ac14443c214bfd0`, on Apple M4 Pro, 48 GiB,
macOS aarch64, Rust 1.95.0. Both sides use release opt3 and all public features. The ignored ledger
and raw artifacts are under `target/bench/experiments/sequence-css-owner-20261019/`.

The question, immutable baseline executables, controls, four memory metrics and gates were
registered before production edits. The existing State probes supplied 402 successful controls
and the 418-cell catalog baseline. Another 86 Sequence controls completed public preflight,
one warmup, five allocation samples and ordered custom-measurer recording before editing.
Twenty-six inherited public parse errors remain a separate lane; none of the new inputs failed.

`SequencePreparedCss` previously copied values from `SequenceTypographyPlan`, its compatibility
binding, the terminal theme and activation theme. The writer now accepts `SequencePreparedArtifact`
and borrows those same owners. The remaining preparation stores the typed-font source marker,
three formatted numeric overrides and the existing seven text surfaces/groups. No cache, new
wrapper, self-reference or extra owner is introduced.

Font size and both line widths keep their original preparation-time formatting and units.
Root and diagram fonts retain distinct raw fallback sources. Loop stroke may be typed while its
fill remains the raw label-box border. Three activation baseline rules still precede typed
overrides. Number/keyword receipts and actor-glyph overrides use the original typed Option
presence, including equal-valued defaults. CSS templates, fragment order, scoped filter handling
and checkpoints remain unchanged. Seven foreground/declaration copies are a separate candidate.

## Exact public measurements

Each complete public render uses deterministic SVG output and keeps the rich result alive at
the memory checkpoint. Theme compilation, source loading and result serialization are outside
the measured operation. Counts include the allocator's allocation/reallocation observations.

| Control | Allocations before → after | Allocated bytes before → after | Peak growth before → after | Retained growth |
| --- | ---: | ---: | ---: | ---: |
| Sequence medium | 4,612 → 4,598 | 746,723 → 746,226 | 288,912 → 288,415 | 60,271 |
| Sequence tiny | 2,362 → 2,348 | 511,589 → 511,092 | 248,283 → 247,786 | 40,493 |
| Rich classic, no theme | 3,434 → 3,420 | 587,071 → 586,574 | 255,610 → 255,113 | 39,025 |
| Rich classic, combined typed theme | 6,763 → 6,749 | 1,286,462 → 1,285,986 | 470,150 → 470,150 | 41,785 |
| Rich classic, typed fonts | 6,632 → 6,617 | 1,269,864 → 1,269,385 | 470,150 → 470,150 | 41,915 |
| 64 messages, no theme | 8,597 → 8,583 | 1,145,318 → 1,144,821 | 360,022 → 359,525 | 80,479 |

Allocation savings depend on the selected properties. The expanded empty/paint definitions also
select both activation overrides, removing sixteen copies; typed font size or message width keeps
its existing formatted allocation, giving fifteen in the corresponding controls. Counts are
observations for these recipes, rather than one universal saving for every typed theme.

All 488 successful controls retain exact SVG, report and ordered measurement trace. Of 347
jointly stable memory rows, 95 improve and 252 remain unchanged; none increase any of the four
metrics. The 141 variable rows remain inconclusive for memory: 134 State and seven Flowchart
controls. `flowchart_backedges_subgraphs` is newly variable on the candidate side; its raw samples
are retained without retry. All 26 inherited errors match exactly.

The 38-scene catalog covers 35 logical families and ten presets plus no theme, for 418 cells.
All warmups/samples, SVG/report bytes, configuration, diagnostics, theme satisfaction and family
identity match. Every memory row is stable. Eleven Sequence cells save 13–14 allocations and
497–571 bytes; the other 407 cells are unchanged. Peak growth never increases, and retained
growth is unchanged everywhere. Catalog and site-control totals differ because their request
and retained-result recipes differ; they are compared only within their own lanes.

The allocation gate explicitly requires both allocation count and allocated bytes to decrease
for Sequence medium, with all four metrics nonincreasing on every jointly stable control and
catalog cell. Independent source and raw-evidence reviews supplement the executable comparisons.

## Historical attribution and limits

v0.9.0 constructs a presentation theme during emission and borrows its Sequence fields; its
configuration conversions, common/family clones and fallback Strings already allocate. Commit
`f495a7542` moves the compatibility reader into operation preparation; that stage/lifetime change
does not prove the whole binding is additional work. `b997dc160` introduces another prepared
scalar owner alongside the original values, although some number/frame/keyword/text ownership
already existed in writer receipts. `9a297b72f` has already removed a separate baseline-fill copy.
This change removes the present duplicate owners; it does not attribute the entire release gap
to them. The removed `Arc::clone` incremented a reference count, rather than copying the raw
binding or allocating another Arc.

## Validation

The focused release nextest run passes 326 tests, including the exact/one-byte-short SVG budget,
failed sink, scoped IDs, custom measurement, strict unsupported facets and math paths. Tests
also distinguish root/theme font sources, Loop typed stroke/raw fill and activation cascade order.
The full renderer release nextest run passes 5,207 tests with five skipped; release all-feature
Clippy passes. The full 37-entry DOM comparison passes structure, parity and parity-root modes
with the existing exact browser-text-layout receipts. The full optimized WASM package builds,
including wasm-opt; WASM runtime smoke was not repeated. Formatting and diff checks pass.

The all-family v0.9.0 goal remains active. The next independent shared-writer experiment should
test whether inlining the successful state check and isolating cold error conversion removes
the real call setup seen in frozen machine code. It must retain bounded capacity, fragment
accounting, cancellation positions and first-error behavior, and pass calibrated public timing.
