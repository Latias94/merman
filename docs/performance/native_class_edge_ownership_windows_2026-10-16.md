# Class edge emission ownership on Windows — 2026-10-16

## Decision and scope

Retain the Class edge producer/consumer ownership refactor without a latency claim. Paths and
labels now share one ordered borrowed edge sequence. Label-center keys borrow immutable edge IDs,
and terminal style remains borrowed until typed styling requires an append. The existing streaming
sink remains; no old temporary SVG buffers are restored.

The product baseline is `ce8ab9185`, the independently verified default-binding repair. The edge
candidate changes only `svg/parity/class/edge.rs` and `groups.rs`. Its patch SHA-256 is
`c53eec8bc68c5dd2ea780378a4dc1d3934436915d99410e1abe787b6488cd551`. Fixed-prefix search and
compatibility-query candidates are excluded. The native artifacts use Windows 11 / Intel Core
i7-11700, Rust 1.95.0, locked optimized benchmark profile, default features plus SVG.

## Preserved behavior

The sort comparator, stable tie behavior, note numbering, relation indices and fallback order
remain. Both production callers retain the same immutable edge collection between phases. The
borrowed center map keeps content-key equality and last-value replacement for duplicate IDs.
Line-hop rewriting and its charging stay at the original point. Typed declarations retain their
append order and punctuation. Write fragments, resource accounting, receipt observations, first
errors and cancellation checkpoints are unchanged. Independent source review passes.

Sorting remains O(E log E) and scratch remains O(E). This removes duplicate sorting and transient
ownership, not a change in complexity order or a proven large source of the release regression.
The shared vector remains alive across the phase gap; it does not retain owned copies of edges.

## Allocation control

Reuse the checked-in counting allocator in an ignored standalone public-family probe. Parsing,
preparation and session creation occur outside counting; complete family `render_svg` occurs inside.
The rendered result remains alive through the snapshot; hashing and destruction occur afterward.
Each side has two warmups and three identical observations per fixture. These are allocation
request volume, peak growth and net retained delta, not RSS or complete-facade memory.

| Fixture | Metric | Baseline | Candidate |
| --- | --- | ---: | ---: |
| Class medium | Allocation count | 1,358 | 1,329 |
| Class medium | Allocated bytes | 382,494 | 381,466 |
| Class medium | Peak growth bytes | 130,450 | 130,416 |
| Class medium | Retained delta bytes | 62,751 | 62,751 |
| Class tiny | Allocation count | 95 | 95 |
| Class tiny | Allocated bytes | 65,756 | 65,756 |
| Class tiny | Peak growth bytes | 30,637 | 30,637 |
| Class tiny | Retained delta bytes | 20,115 | 20,115 |

The medium fixture has 14 labeled relations. Its 29 fewer allocations agree with removing 14 ID
owners, 14 unconditionally owned terminal styles and the second ordering vector. The 1,028-byte
reduction is small and does not explain the large release latency gap. Tiny has no edges and all
four allocation metrics remain equal. Both fixtures have identical SVG byte counts and SHA-256.

## Validation and latency limitation

The frozen native benchmark executable grows from 40,271,360 to 40,347,648 bytes: +76,288 bytes,
about 0.19%. Retention therefore accepts a small native artifact-size cost in exchange for clearer
producer/consumer ownership and less repeated work. This is not an all-metric improvement, a
WASM size result or an inference about other packaging surfaces.

The full renderer nextest run passes 3,321 tests with 2 existing skips, covering all-diagram, ELK,
Cytoscape and math features. It includes a new stable ordering boundary test. Workspace formatting
and renderer all-target Clippy with `-D warnings` pass. The frozen pipeline smoke produces 459
preflight receipts per side; all 152 render/end-to-end SVG identities match. Unordered non-SVG
State projections remain the previously recorded caveat.

The seven-fixture three-pair diagnostic is rejected before its first measurement: the native
process snapshot observes external Cargo/rustc. It has zero admitted rows. No latency improvement
or representative latency non-regression is established, and no samples are appended. The retained
change is an ownership/work-duplication refactor; further public timing needs a new controlled
experiment. No host process or policy is changed.

## Local receipts

The ledger, separate patch, exact source/probe/extern hashes, Cargo artifact JSON, frozen baseline
and candidate executables, raw allocation observations, full-corpus identities and rejected timing
receipt remain in `target/bench/experiments/class-ownership-20261016/`. The allocation script verifies
the same source and extern profile identities for both sides. The standalone probe does not add a
regular CI performance gate.

Continue with borrowed compatibility queries and fixed-prefix reserved-token search as separate
causal candidates. The main remaining latency attribution target is mandatory standalone
finalization; the current public render-stage benchmark combines preparation and emission and
must not be described as pure emission or as complete facade finalization.
