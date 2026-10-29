---
title: Preserve label stages and independent rendering evidence
type: refactor
date: 2026-10-28
---

# Text stage contracts

## Scope

Follow up merged PRs #246 and #247 on new branches based on main and release/0.9.x.
Repair the independently reproduced State, Usecase, Kanban, Class retained-memory,
browser-measurement, and maintenance native-math defects. Complete the existing
Event Modeling work without changing its pinned layout-probe semantics.

## Design

- A prepared family label owns its terminal content, effective configuration,
  geometry, and any retained-memory reservation.
- Source markup, a pinned layout probe, SVG words, and serialized XHTML are
  separate semantic stages. A shared owner may produce several explicit
  projections. It must not reinterpret final characters as authored markup.
- Event Modeling measures the generated HTML source required by upstream
  calculateTextDimensions, retains that result, and emits prepared XHTML.
  Entity serialization must not change the layout probe.
- Class reservations live as long as the shared retained plan. Preparation,
  failed operations, cancellation, cloning, and destruction must agree on
  ownership rather than relying on a one-time admission check.
- Browser adapters implement each measurement operation's DOM shape and
  transformations. Valid subpixel sizes remain valid; an unsupported operation
  returns unhandled instead of claiming an approximate replacement.
- Native math plus caller CSS must preserve producer paint or explicitly reject
  the unsupported combination. Ordinary non-math CSS export remains available.
- Cross-runtime equality does not prove text or formula existence. Add independent
  formula-ink and partial-clipping controls to the existing evidence owners.

## Work and audit boundaries

Six workers own State, Kanban/Usecase, Web adapters, validation, native math/CLI
tests, and Class accounting. The coordinating worker owns Event Modeling and
shared family preparation. Each worker audits other consumers of its changed
contract and reports concrete counterexamples; speculative observations are not
converted into broad rewrites.

Preserve unrelated work and the old worktrees. Reuse the existing target directory,
run Cargo serially with two build jobs, and keep dependency/feature changes
explicit. Do not relax comparators, root receipts, or numeric tolerances.

## Verification and delivery

1. Add public-consumer regressions for each confirmed defect and source-backed
   controls that distinguish literal text from markup.
2. Run focused Rust nextest owners, Web/browser tests, and Python contracts.
3. Run formatting, source hygiene, strict lint where applicable, and the required
   complete SVG structure gate. Regenerate only justified source-owned baselines.
4. Independently review the integrated ownership and semantic boundaries.
5. Commit logical units and open linked main and maintenance PRs with accurate
   behavior changes, evidence, and explicitly bounded residuals.

Performance optimization remains paused. Correctness checks do not establish a
performance claim.


## Completion record

The planned family repairs, protocol projections, maintenance HTML5 backport,
independent evidence controls and bounded cross-family audits are implemented.
[The research record](../research/2026-10-28-text-stages.md#verification-record-2026-10-29)
names actual passing owners and the maintenance macOS root-fingerprint boundary.
Windows preparation, independent documentation-lock security, CSS EOF fuzzing,
parser recovery policy and main-only historical performance-contract handling are
separate commits. Linked main/maintenance pull requests carry the final review
and remote CI evidence; no release or automatic merge is part of this work.
