---
type: Work Progress
title: Borrow canonical family fields in terminal adapters
description: Share Class topology, State graph views, Gantt date facts, and XYChart and Mindmap display rules while preserving existing full terminal output.
timestamp: 2026-10-21T23:47:18Z
record_id: 3ec20b85-5ad6-4eb3-986d-91d475cdb75f
producer_id: codex-root
run_id: semantic-capability-boundaries-u6
related_plan: docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md
git_branch: refactor/semantic-capability-boundaries
git_commit: fdbadda3c
---

# Family Ownership

The existing full ASCII dispatch now uses family adapters that borrow the
canonical fields they need. Canonical/full comparison tests exercise the same
adapters; no whole-model conversion or second family parse is added.

- Class has one `ClassTopology` owner for effective namespaces, parent choices,
  namespace route identities, and collision-free note/interface routing. A
  privately paired `ClassTopologyView` borrows canonical classes, relations,
  notes, interfaces, and style witnesses. The full view borrows its already
  constructed topology; its class DOM map remains presentation-only. Full-wire
  deserialization retains supplied identities and invalid ownership evidence
  rather than reconstructing and silently repairing them.
- State exposes Copy borrowed graph views over either canonical or full nodes
  and edges. Terminal code reads no DOM IDs, dimensions, or palette. Detached
  label-style precedence and invalid-arrow rejection remain supported, and
  note/divider/pseudo-state, direction, parent, and endpoint validation is shared.
  Temporary group ordering stores indices instead of copying entire node views.
- XYChart shares its existing Copy display policy outside presentation. The
  terminal view borrows its canonical chart and that policy, preserving all
  eleven title, axis, and data-label flags, including empty-chart behavior.
- Gantt retains its excludes-visible end in the existing date-adjustment pass.
  Canonical tasks keep the scalar endpoint; the full wrapper copies that same
  value and still permits explicit detached overrides. Terminal task indexing
  remains borrowed, with no cloned task vectors. The canonical field is skipped
  by existing serde output, so a canonical wire round trip does not retain this
  auxiliary fact; full wire output retains its existing outer `renderEndTime`.
- Mindmap shares shape and section rules with graphical projection. The terminal
  adapter borrows the authored tree and validates IDs, missing children, parallel
  edges, and multiple parents. Disconnected cycles remain visible and terminate.
  Its index and traversal frames store source indices, not cloned nodes. Numeric
  identities preserve old ASCII scan work without temporary formatted strings;
  canonical edge-count and edge-visit scans pay incremental work and checkpoints.

Mindmap keeps the original eleven-slot branch cycle, traversal-depth color mode,
and case-insensitive Redux default shape. The shared policy reads that theme name
without allocating a lowercase copy. This is an implementation property, not a
measured whole-operation allocation or latency claim.

Rust source migration must be recorded in U8: exhaustive `GanttTask` literals
must initialize `render_end_ms`. `ClassPresentation` keeps borrowed and mutable
field access through its topology owner, but moving an owned topology field now
requires an explicit extraction such as `std::mem::take`. Existing wire schemas
are unchanged; these Rust ownership changes are not claimed to be source-compatible.

# Review Corrections

Independent read-only reviews covered Class, State/Mindmap, and XYChart/Gantt.
The Class review identified a real allocation regression in flat namespace mode:
the new loop initially reserved slots for all implicit namespaces before filtering
them. Flat mode now starts with an empty effective map, matching the previous
filtered collection behavior. A 128-level namespace test checks that only the
effective container needs capacity, that the canonical route ordinal remains
127, and that an all-implicit effective map retains zero slots. This does not
claim allocator RSS or an end-to-end memory measurement.

The initial compile also found one stale Class test call; it now keeps a local
topology-view binding for the borrowed relation-label result.

The initial full test run exposed a pre-existing flaw in the preceding recovery
test: complete Debug strings compared randomized State HashMap order. Commit
`fdbadda3c` compares the two unordered maps by key, destructuring every State
field exhaustively and retaining all document, node, edge, and array order.
It changes neither the parser nor the diagnostic comparator. Twelve stress
iterations and the complete test rerun pass. No tests or fields were dropped.

# Verification

All Cargo builds ran serially with two build jobs, incremental compilation off,
and the single shared `D:/merman-u5-temp/verification-target`. Tests used two
threads. No additional worktree or repository target was created.

| Selection | Result | Nextest run ID |
| --- | --- | --- |
| core and ASCII, all features, lib and tests | 3,684 passed, zero skipped | `afe2079f-21c6-417c-8c1e-a9b2854ba92f` |
| Seven Class/State/Gantt/Mindmap/XYChart SVG and layout binaries | 408 passed, zero skipped | `43b0f629-eb22-413d-a145-e862c842da18` |
| core, only the five family selectors, without presentation, JSON, or editor facts | 265 passed, zero skipped | `a499bc2a-fee9-4cb4-ab78-347f4094cb1a` |
| Recovery-equivalence test after keyed-map correction | Twelve of twelve stress iterations passed; other tests intentionally filtered | `c4f918bb-8901-4cb2-b928-acc3bca0a953` |

Strict all-target Clippy passes for full-feature core and ASCII with
`--keep-going -- -D warnings`. The five-family lean core selection still emits
thirteen production warnings and additional test-helper warnings; the standalone
renderer selection without editor facts emits eighty core warnings. Those
feature-specific configurations pass their tests but are not strict-lint claims.
No broad allowances or ignored tests were introduced. Logs remain under
`D:/merman-u5-temp/u6-terminal-*`.

# Remaining Integration

This is a family-ownership unit, not completion of the ASCII capability boundary.
The public ASCII entry still accepts the full enum, its normal manifest still
selects presentation/compatibility JSON, and complete overflow fallback still
uses legacy projections. Canonical/full tests do not substitute for a final
independent ordinary-consumer feature-closure check.

Class canonical topology construction is cancellable but its output-time work
and storage requests still need ASCII admission before allocation. Retained bytes
after construction are not sufficient. The next integration should add only a
Class-local fallible incremental admission callback to the existing constructor:
ASCII forwards actual slot, traversal, and copied/generated-byte work to its
existing `ResourceContext`; full parsing uses the same constructor without the
ASCII callback charge. Avoid a worst-case N-squared precharge, a second topology
replay, a new budget registry, or converting output-byte limits into RSS limits.
Keep the topology alive across layout candidates and fallback, and retain sticky
terminal errors plus the existing speculative-document rollback contract.

The unified canonical ASCII entry and complete family fallback, facade/prepared
artifacts, and host consumers remain open under U6. U8 naming/package alignment
and U9 final correctness, ABI, allocation, and matched stripped-size evidence
also remain required. The approved goal stays active. Nothing was pushed or
published.
