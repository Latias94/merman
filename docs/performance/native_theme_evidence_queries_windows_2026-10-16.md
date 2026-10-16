# Borrowed compatibility evidence queries on Windows — 2026-10-16

## Decision and scope

Retain the query ownership repair. It eliminates owned compatibility snapshots for admission recipe equality and
ordinary residual counting. Both consumers now query the same frozen metadata directly. The owned
snapshot API remains for consumers that need field/path identities, including Packet reconciliation.
This is a resource-work and allocation-bound repair. Ordinary latency remains a separate gate.

The adjacent product baseline is `92ec3ad4c`, including the independently verified Class changes.
The frozen candidate consists of the five-file patch
`6a4bd7fce6843b6aa3015489de92a10bc68b32248ad3de0cdda0d0049f0cb9a0`;
reserved-token search is excluded. Its artifact is built from a dirty working copy with exact source
hashes, not presented as a clean-ref confirmation. The host is Windows 11 / Intel Core i7-11700,
Rust 1.95.0, locked optimized native benchmark profile, default features plus SVG.

## Ownership and equivalence

Admission previously built a complete `ThemeParseEvidence` only to compare its recipe. Ordinary
family preparation then built it again only to read the residual count. Each snapshot maps frozen
fields into new evidence, rebuilding opaque-ID/path owners and an Arc slice. Even empty evidence
owns a fresh Arc header. The new hidden bridge compares the original complete `ThemeParseBinding`
and reads the original frozen field slice length.

Recipe comparison preserves identity and compatibility-value equality, including None/Tracking/
Frozen distinctions. It is still before resource preflight and retains existing error ordering.
Residual count does not infer emptiness from the absence of a selected theme. Packet still creates
and consumes complete evidence in its original family branch; terminal reconciliation is unchanged.
No parser, cache, policy bypass, report reduction, equality shortcut or dependency is introduced.

With F frozen fields, P surviving assignment paths, I opaque-ID bytes and recipe comparison cost R,
the old consumers perform two O(F+P+I) snapshot constructions plus R. Their new extra snapshot
work is zero; recipe equality retains R and counting is O(1). Additional query space is O(1).
This removes input-scaled materialization from consumers that only need predicates/counts;
it does not remove the full owned API or change whole-render complexity.

## Same-binary allocation control

A standalone optimized probe links the candidate core library recorded by Cargo artifact JSON.
The original owned API and new borrowed queries run in the same executable. Parse, plans, metadata,
labels and expected results are built before counting. The original path constructs separate
snapshots for its two consumers, matching production. Its temporary snapshots are destroyed before
the snapshot ends, so allocation and free work are both counted. The new result is a boolean/count
pair. Both paths use black-box inputs/results and agree exactly.

Each of six inputs has two warmups and three retained observations per side, all identical. The
128-field input is below the existing 512-variable compatibility bound. Metric cells are owned /
borrowed. These measure only the two consumer queries, not the full public prepare operation or RSS.

| Input | Residual fields | Allocation count | Allocated bytes | Peak growth bytes |
| --- | ---: | ---: | ---: | ---: |
| default | 0 | 2 / 0 | 32 / 0 | 16 / 0 |
| fields-0 | 0 | 2 / 0 | 32 / 0 | 16 / 0 |
| fields-1 | 1 | 10 / 0 | 384 / 0 | 176 / 0 |
| fields-3 | 3 | 22 / 0 | 1,088 / 0 | 496 / 0 |
| fields-32 | 32 | 196 / 0 | 11,296 / 0 | 5,136 / 0 |
| fields-128 | 128 | 772 / 0 | 45,088 / 0 | 20,496 / 0 |

Net retained delta is zero on both sides for all inputs: the old snapshots were temporary.
All borrowed cases have zero allocation requests, zero allocated bytes and zero peak growth in
this scope. Default empty input saves two allocations and 32 bytes; F=128 saves 772 allocations and
45,088 bytes. These are bounded consumer measurements, not an attribution of the main default
latency regression. Mismatch, Tracking and shadow semantics are covered by the separate 7-state /
6-recipe differential test against the unchanged owned API.

## Correctness, tooling and size

The resumed full core/renderer nextest run passes 5,163 tests with 2 existing skips, including the
new query oracle test and existing parse/session mismatch, authority, Packet, resource and cancel
contracts. The interrupted first log has no completion and is preserved without a pass claim.
Workspace formatting passes. Core all-target Clippy passes with `all-diagrams,test-support`;
renderer all-target Clippy passes with all diagrams, ELK, Cytoscape and math, both using
`-D warnings`. Earlier incomplete feature combinations exposed existing test-only unused items;
those failures remain recorded, and no unrelated production/test code is changed to silence them.

The complete native pipeline smoke yields 459 preflight receipts per side. All 152 render/end-to-end
SVG identities match exactly. Unordered non-SVG State projections remain a separate recorded
reproducibility caveat.

The frozen native benchmark executable grows from 40,347,648 to 40,518,656 bytes: +171,008 bytes,
about 0.42%. This accepts a native artifact-size cost for removing duplicated input-scaled owners;
it is not a claim that all metrics improve, nor a WASM/package-size result.

## Latency and local evidence

The complete three-pair seven-fixture diagnostic records 42 measurements and 84 clear native
process boundary snapshots. SVG pre/postflight identities remain exact. It is a frozen dirty-patch
diagnostic, without A/A calibration or a power-sized clean-ref confirmation:

| Fixture | Base us | Candidate us | Change |
| --- | ---: | ---: | ---: |
| class_medium | 2512.700 | 2584.233 | +2.85% |
| journey_medium | 203.573 | 206.137 | +1.26% |
| class_tiny | 550.803 | 558.950 | +1.48% |
| xychart_medium | 374.793 | 376.017 | +0.33% |
| flowchart_medium | 3507.300 | 3387.900 | -3.40% |
| sequence_medium | 1065.800 | 1070.967 | +0.48% |
| kanban_medium | 268.407 | 264.643 | -1.40% |

These mixed movements establish neither improvement nor representative latency non-regression.
No ordinary fixture clears both 10% and 50 us even by its uncalibrated mean. Keep the resource-work
repair because its exact allocation/bound exit passes, not because this diagnostic proves a speedup.
The unchanged latency gate still requires canonical clean-ref A/A and power-sized confirmation.
No samples are appended to this frozen run.

Sources, frozen binaries, exact extern/source/allocator hashes, Cargo JSON, 36 raw allocation rows,
identity logs, test/Clippy logs and timing receipts remain under
`target/bench/experiments/theme-evidence-queries-20261016/`. The allocation probe uses public
`MermaidConfig::set_value` for setup and links only core. Its first pre-build receipt selection
encountered two serde_json versions; that failed before compiling/running and creates no samples.

Continue separately with the mandatory reserved-prefix observation and head-only standalone
finalization attribution. Existing validation, resource accounting and terminal evidence remain
requirements of that work.
