# Class shared default bindings on Windows — 2026-10-16

## Decision and scope

Retain the Class default-binding ownership repair. It removes the measured node-dependent
copy of shared default style payload during preparation. It does not establish a complete-operation
latency improvement, whole-process RSS improvement, or recovery to v0.9.0.

The source baseline is `0f368910e`; root `0ae082339` adds only performance documentation.
The candidate changes only `class/node_binding.rs` and `svg/parity/class/nodes.rs`. Separate
edge-ordering, reserved-token search and evidence-query candidates are excluded from these artifacts.
The retained experiment uses the same Windows 11 / Intel Core i7-11700 host and Rust 1.95.0,
locked native optimized benchmark profile, default features plus SVG.

## Ownership and semantic contract

Class preparation previously cloned fill, stroke, width and dash into each node, including defaults
already held by its artifact. The candidate stores only actual source/typed overrides and selects
shared artifact defaults at emission. Explicit overrides equal to defaults remain owned. Source and
typed authority are not deduplicated by CSS equality; an explicitly empty width stays empty.

The artifact constructs and consumes the visual plan and immutable typography/CSS binding together.
No sibling self-reference, global cache, new parser or family bypass is introduced. The original
terminal observation, write fragments, resource checks, completion, cancellation and first-error
ordering remain. Independent source review verifies the private artifact/context pairing.

## Preparation allocation control

Reuse the preceding public `family::prepare` probe without changing its source or allocator.
Parse, session creation and parsed-input cloning occur before counting; the prepared result remains
alive at the snapshot. Rendering, SVG hashing and destruction occur outside counting. Each of six
shapes has two warmups and three allocation observations; all three observations are identical per
side and shape. This measures allocation requests, peak growth and net retained delta of that
scope, rather than absolute artifact size or process RSS. Allocated bytes include requested
reallocation sizes, and net retained delta can include consumed-input frees.

C is the node count. K is additional ASCII whitespace inside an authored raw `nodeBorder` CSS token;
the input stays below 16 KiB. HTML labels remain enabled. Exact SVG byte counts and SHA-256 identities
match the baseline in all six shapes, including output above 1 MB.

Each metric cell is baseline / candidate; sizes are bytes.

| C | K | Allocation count | Allocated bytes | Retained delta | Peak growth |
| --- | --- | --- | --- | --- | --- |
| 16 | 0 | 2,546 / 2,482 | 217,494 / 217,126 | 22,302 / 21,934 | 52,092 / 52,092 |
| 16 | 1024 | 2,546 / 2,482 | 237,974 / 221,222 | 42,782 / 26,030 | 56,188 / 56,188 |
| 16 | 4096 | 2,546 / 2,482 | 299,414 / 233,510 | 104,222 / 38,318 | 104,236 / 68,476 |
| 64 | 0 | 9,074 / 8,818 | 991,542 / 990,070 | 73,566 / 72,094 | 201,132 / 201,132 |
| 64 | 1024 | 9,074 / 8,818 | 1,061,174 / 994,166 | 143,198 / 76,190 | 205,228 / 205,228 |
| 64 | 4096 | 9,074 / 8,818 | 1,270,070 / 1,006,454 | 352,094 / 88,478 | 352,108 / 217,516 |

Both allocated and retained padding coefficients fall from `C+4` to exactly 4 at C=16 and 64,
for K=1024 and 4096. Default allocations fall by exactly 4C. At C=64/K=4096, net retained delta
falls from 352,094 to 88,478 bytes (74.87%); peak growth falls from 352,108 to 217,516 bytes.
At K=0, peak growth is unchanged. The shared defaults still have fixed additional ownership;
this experiment does not attribute every fixed copy or remove that independent frontier.

The removed extra prepared default-token payload is O(CK); shared payload is O(K), plus unchanged
O(C) binding selections. The final SVG still repeats tokens and remains Omega(CK). This is a
pre-emission retained-payload repair, not a reduction in whole-render time complexity.

## Correctness and tooling

The seven node-binding tests pass, including long defaults borrowed by pointer, explicit same-value
source ownership, typed/source authority, empty trimmed width, prepared facts and cancellation.
The full renderer nextest run passes 3,320 tests with 2 existing skips, using all-diagram, ELK,
Cytoscape and math features. Workspace format checking and renderer all-target Clippy with
`-D warnings` pass. The ordinary native pipeline smoke produces 459 complete preflight receipts
per executable; all 152 render/end-to-end SVG identities match exactly. Unordered non-SVG State
projections remain the previously documented reproducibility caveat.

The probe and baseline executable are frozen. Cargo artifact JSON identifies the optimized
libraries, and the probe records exact extern library hashes. Two sha2 versions occur in the
closure; the receipt selects the exact linked rlib by hash rather than target name alone.

## Latency limitation

No latency samples are admitted. The first diagnostic fails before its first measurement because
`tasklist` times out. The preregistered single fresh retry uses native `Get-Process` JSON and
observes external Cargo/rustc processes at its first boundary; it is rejected with zero samples.
The failed artifacts remain. No user process or host policy is changed, and no third attempt is
appended. Representative latency non-regression remains unconfirmed; no timing claim follows
from removing allocations. A later controlled public comparison must use a fresh registered
schedule and the unchanged admission gate.

## Local evidence and next work

All ledgers, sources, patches, build receipts, frozen executables, raw observations, identity
receipts and failed diagnostics remain under `target/bench/experiments/class-ownership-20261016/`.
The source probe SHA-256 is `9b2e45de739aefb0771dafa3cf74a12b8e24a67b89359e517f43b14629342523`;
the candidate pipeline SHA-256 is `a631145ca2082ad0422061f28eebcf65adb6a540794be685b1d4b329b1b74970`.

Continue independently with shared Class edge ordering, fixed-prefix reserved-token search and
borrowed compatibility queries. The larger complete XML/reference admission frontier remains
open. Preserve all validation and resource contracts while attributing the remaining latency gap.
