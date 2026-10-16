# SVG operation-policy borrowing on Windows, 2026-10-15

The reviewed ownership change was committed as `0f368910e` on October 16. Measurements below
retain their original precommit diagnostic scope; no latency improvement is admitted.

Base: `ad3d5f07bcb5dd6ee6d1cf244727835341193c68`. Candidate: frozen uncommitted
three-file ownership refactor, patch SHA-256
`c8c68182779d12661bb4552d3496d80e5918c6e88699e60fd1899e1ac2811764`.

## Question and ownership change

The raw SVG structure callbacks obtained `RenderResourcePolicy` by value for every element,
although both element-count checks already borrow it. The operation's immutable `RenderSession`
owns the policy. A crate-private session getter now borrows it, and the existing private
`SvgPostprocessExecution` getter carries that reference through its session lifetime. The static
dimension helper accepts the same reference. Public by-value getters remain compatible.

No XML check, resource counter, error construction, termination call or cancellation checkpoint
was removed or reordered. This is a shared ownership change across all four controlled validation
entries, without family classification, parser bypass, cache or extra proof state.

The per-element policy-copy/check contribution remains O(E), excluding the unchanged XML and
reference work. Removing `488 * E` copied bytes in the inspected raw structure callback is a
constant-factor work reduction, not an asymptotic repair or an allocation reduction.
Class medium has 576 elements (281,088 copied bytes per raw walk); Journey has 96
(46,848 bytes). This evidence alone cannot establish a material public-operation improvement.

## Generated-code exit

The actual ad3d callback `c8775c2644646882…2555715477728552159` calls `memcpy` at
`+0x2d` with length `0x1e8`, before checking whether an element ceiling exists. Both XML validation
entries reference that closure. The separate error-only override-iterator copy is not this target.

Candidate generated-code exit passes. The callback directly reads the session's policy ceiling;
its success path has no call, allocation or policy copy. Its local stack reservation decreases
from 968 to 480 bytes. The only remaining memcpy is the 160-byte error-only override-iterator
copy. Checkpoint callback and both validation-entry instructions/relocations are identical after
normalizing the LLVM identity suffix, so the work has not merely moved to the entry or checkpoint.

The baseline executable is the prior frozen product-source-matched ad3d executable, not a new
clean-build confirmation. Its inspected current rlib lacks a retained original Cargo artifact
receipt/link map, so base object evidence is not exclusive CPU attribution. Candidate build JSON
retains the rlib and benchmark artifact names from one serial Cargo build.
Those receipts do not replace a PE link map or sampled CPU attribution.

## Verification

All 3,317 renderer library tests pass under nextest; two existing tests remain skipped. This
all-diagrams/ELK/Cytoscape/math test lane uses the release dependency graph with renderer
`opt-level=0`, and is separate from the optimized measured executable. Renderer all-target Clippy
with those features and `-D warnings`, workspace formatting and independent semantic review pass.

All 152 SVG rows match exactly, with 459 complete preflight receipts per executable. Five
unselected State parse/compatibility/layout projection receipts differ between executables.
Repeating the full smoke twice per same frozen executable also changes State projection receipts
on both sides, including the baseline. This is an independent reproducibility issue; no parse or
layout identity/latency claim is made. The initial exact-filter State smoke exited 101 and is
retained as a failed observation, not used as passing evidence. The native
benchmark executable decreases from 40,271,872 to 40,270,336 bytes (1,536 bytes). This is the frozen
pipeline benchmark artifact size, not a packaged CLI/WASM size claim.

## Measurement boundary

Windows 11, Intel Core i7-11700, Rust 1.95.0, x86_64-pc-windows-msvc. Both benchmark artifacts use
the locked native bench profile, default features plus `svg`, and the same harness/corpus/fixtures.
The baseline binary is frozen before the candidate build. Timing uses no Cargo invocation.

Seven complete `end_to_end` fixtures use three AB/BA diagnostic pairs, 30 Criterion samples,
two-second warmup and three-second measurement. Side values are arithmetic means of independent
console point estimates. These are diagnostic observations, not calibrated confirmation or
latency admission. The ordinary joint gate remains 10% and 50 us; no threshold is relaxed.

Each measurement checks complete pre/postflight identities and before/after process snapshots for
Cargo, rustc, link and cl. A detection, snapshot or evidence-write error stops the run and marks its
partial report failed. These snapshots cannot establish absence of mid-interval compilation or
host isolation. User processes are never terminated.

The first attempt detected cargo.exe PID 100436 after first-round candidate XY Chart and
stopped with seven complete rows. Its entire run is rejected and archived separately; the process
had already exited before its command could be captured. Later snapshots observed other Cargo
license-generation work. No failed-attempt samples are used below. One preregistered fresh retry
completed all 42 measurements and 84 clear boundary snapshots, with unchanged executable/patch
hashes and exact selected pre/postflight SVG identities.

| Fixture | Base (us) | Candidate (us) | Delta (us) | Candidate / base change |
| --- | ---: | ---: | ---: | ---: |
| class_medium | 2692.267 | 2641.933 | -50.333 | -1.87% |
| journey_medium | 226.767 | 225.343 | -1.423 | -0.63% |
| class_tiny | 709.530 | 593.563 | -115.967 | -16.34% |
| xychart_medium | 449.713 | 413.057 | -36.657 | -8.15% |
| flowchart_medium | 3706.167 | 3868.500 | +162.333 | +4.38% |
| sequence_medium | 1200.033 | 1210.700 | +10.667 | +0.89% |
| kanban_medium | 287.333 | 281.850 | -5.483 | -1.91% |

Changes are mixed. Class tiny has a favorable point estimate, but this run contains no A/A
calibration or power-sized confirmation. Class medium improves only 1.87%, Journey is essentially
flat, and Flowchart has an unfavorable +4.38% point estimate. Neither improvement nor
non-regression is admitted. This candidate does not close the historical SVG regression.

At measurement time the ownership refactor was a reviewed, uncommitted candidate. Its generated-code exit is
proven; public latency admission remains inconclusive. A clean-ref confirmation on a stable host
would be needed before claiming a measured speedup. Do not append samples to this frozen run.

## Remaining owner boundary

The shared byte sink currently receives raw markup from all 35 dispatch variants through 33
producer modules. Eliminating parse-after-generation requires typed emission plus complete
facts carried through root mutation, family artifacts and final admission. Eight shared files
cover five responsibilities. Uncovered raw fragments must invalidate facts and fall back to the
existing parser; a family allowlist or another parser inside the byte sink is not a solution.

A complete Info vertical slice could establish that shared chain before broader migration.
Preserve Usecase final-length admission and SvgDiagramId formatting projection charges: changing
append boundaries or formatting twice changes resource actual/first-failure behavior. This is a
separate architecture frontier, not an implemented optimization. Source evidence is retained in
`generation-boundary-review.md`.

## Local replay evidence

Raw commands, source hashes, patch, build artifact JSON, generated-code evidence, identity and
diagnostic logs are retained under `target/bench/experiments/svg-policy-borrow-20261015/`.
The implementation source is also isolated in `E:/Rust/merman-bench-20261015/policy-owner`.
The root kept the prior performance-document changes. No commit or push was performed during
measurement; the subsequent implementation commit is recorded above. No push was performed.
