# Explicit theme overlay order correction — 2026-10-16

The publication review found an internal equivalence gap in the lazy provenance interpreter:
literal dotted roots used longest-prefix fallback, while the original eager interpreter applies
top-level overrides in actual Map iteration order. With `preserve_order`, inserting `a.b` before
`a` can erase the former source during overlay. A later full replay can restore that descendant
when the ancestor's shallow replay does not write it again. Both operations must be represented.
This review establishes an interpreter mismatch, not a reproduced public SVG difference.

The corrected authority indexes both sides of overlapping literal roots with their actual write
ordinals. Reads select the last matching writer; full replay records an overlapping root even
when it was initially shadowed. Computed ancestor barriers and lazy source creation are retained.
The input Map is not reordered, and reads do not scan all explicit overrides.

Let E be the explicit top-level root count and K the roots participating in a dotted-prefix
overlap, including ancestors and descendants. The added index retains O(K) borrowed-key/ordinal
entries, with K <= E and no copied key bytes. Construction retains prefix lookups and adds one
E-root ordinal pass when K is nonzero. Unrelated scalar overrides have K=0. This is a semantic
correction, not an overall linear-complexity or measured-latency claim.

The independent eager oracle covers seven value shapes across all six orders of three roots:
42 cases compare overlay, first-read dependencies, full replay and subsequent reads. Shapes
include leaves, empty/nonempty objects and literal dotted-key collisions. A separate sparse-index
test adds 0/16/256 unrelated dotted roots without adding index entries or source seeds. Existing
0–4,096-variable work-bound tests remain in the suite.

Core nextest passes 2,006 tests with no skips under `all-diagrams,test-support`. Workspace format
and core all-target Clippy with warnings denied pass. Independent source review passes. Raw logs,
the reviewed patch and source receipts are retained under
`target/bench/experiments/pr-theme-repair-20261016/`.
