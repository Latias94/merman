# TreeView optional terminal proof (macOS, 2026-10-19)

TreeView now prepares terminal theme evidence only when a theme was requested. The no-theme path
keeps its existing configuration, font and CSS binding, then skips the source-tree count walk,
layout icon-count walk, expected-font copy and per-node/line evidence recording. Actual rendering,
icon resolution, currentColor handling, output limits and cancellation checkpoints retain their
existing paths.

The proof expectation uses `Option<NonZeroUsize>` in place of the existing `usize`. A model always
contains its root, so a requested theme always has a positive expected line count. This represents
an absent proof without adding another word to the plan. Empty and unsupported requested recipes
retain the complete original counting and receipt path. The old no-theme `proves`/`record_terminal`
result was an ignored boolean with no evidence consumer; skipping it changes no public rejection.
Depth, themed-paint bounds, icon and output errors remain independently enforced.

The registered experiment is under
`target/bench/experiments/treeview-unrequested-proof-20261019/`, based on product `cd7b23674`.
The original 206-control and 418-cell baseline uses the preceding immutable C4 candidate probes;
its production patch digest exactly matches that committed product. Another 25 controls cover all
17 tracked TreeView fixtures and zero/one/16/64-child wide and deep trees. Their baseline uses the
same frozen pre-edit executables. All inputs and selections were registered before candidate
measurement. Source development used a separate worktree while release confirmation was running;
the measured checkout stayed unchanged. Only after confirmation finished was the owned patch
applied and compiled in that original checkout with its existing target directory.

All 231 public controls preserve exact SVG/report bytes and ordered custom-measurer traces.
Across 224 jointly stable memory rows, 25 improve and none regress in allocation count, allocated
bytes, peak growth or retained growth. Six Flowchart rows and the filename/extension-icon TreeView
fixture remain inconclusive for memory; the latter already varies in the immutable baseline.
Their outputs and measurement traces still match.

| Complete public control | Allocations, before → after | Allocated bytes, before → after |
| --- | ---: | ---: |
| TreeView medium | 256 → 253 | 33,101 → 33,022 |
| Root only | 175 → 173 | 21,238 → 21,191 |
| Wide tree, 64 children | 1,657 → 1,654 | 380,827 → 380,268 |
| Deep tree, 64 descendants | 1,838 → 1,836 | 443,573 → 443,526 |

Medium peak growth falls from 14,946 to 14,907 bytes; retained growth stays at 4,111 bytes.
All 418 catalog cells across 35 logical families and ten presets plus no theme remain stable with
exact SVG, reports, configuration, status and catalog metadata. Only the no-theme TreeView cell
changes its memory metrics, saving three allocations and 79 bytes; the other 417 cells are
unchanged. Each probe has its own diagram ID and retained report, so compare absolute byte values
only within the same lane.

Renderer release nextest passes 5,202 tests with five skipped; the public TreeView boundary/native
SVG lifecycle test passes. The 58 focused TreeView tests include requested-theme, icon, callback
and exact output-budget contracts. Release all-feature Clippy, formatting and diff checks pass.
The full 37-entry DOM matrix passes structure, parity and parity-root modes. The descriptor-owned
full browser WASM build, including wasm-opt, passes; package runtime smoke was not repeated.
Independent source review found no semantic or error-contract counterexample. Commands and results
remain in the experiment ledger. This is exact allocation/work-removal evidence; total rendering
remains linear in its input/geometry/output work. No TreeView latency or v0.9.0 convergence claim
is made.
