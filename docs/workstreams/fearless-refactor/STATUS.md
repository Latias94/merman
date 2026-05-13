# Fearless Refactor Status

Snapshot: 2026-05-14

This page is the short-form dashboard for the fearless-refactor workstream.
The detailed plan still lives in `TODO.md`, `MILESTONES.md`, `OVERRIDE_FOOTPRINT.md`, and `COMPLETION_AUDIT.md`.

## Current Read

Overall completion: about 90%.

What is done:

- M0 safety baseline is complete.
- M1 pipeline ownership cleanup is complete.
- M2 typed model expansion is complete for all in-tree Mermaid diagrams except the explicit error/custom-registry fallback path.
- M3 text subsystem modularization is complete.
- M4 large renderer decomposition is effectively complete.
- Root viewport override no-growth is tightened to `432` after the GitGraph seeded auto-id
  warm-up pass, the GitGraph horizontal
  branch-label width pass, the GitGraph commit/tag label computed-length pass, the Flowchart
  imageSquare layout-bounds pass, and the Flowchart anchor layout-bounds pass, the Flowchart C1
  replacement-glyph measurement pass, and the Flowchart SVG-like and Unicode/entities
  subgraph-title/root-bounds passes, followed by two stale Flowchart title-margin root-pin
  deletions, the Flowchart HTML-label font-size precedence derivation, and the Flowchart
  iconSquare layout-bounds and custom FontAwesome fallback passes.
- Sequence layout has been split down to focused actor, activation, block-step, block-bounds,
  note, message, rect, root-bounds, and orchestration owners.
- `cargo run -p xtask -- verify --strict` passes.
- `cargo run -p xtask -- verify --strict` now includes full `parity-root` coverage.
- `cargo run -p xtask -- report-overrides --check-no-growth` passes.
- `cargo bench -p merman --features render` has a fresh post-cleanup release gate record.
- Root `CHANGELOG.md` now calls out the refactor release-readiness work.
- Clippy is part of the strict release gate.
- Hand-curated helper overrides are at `0`.
- Manual raw SVG/path bridge functions are at `0`.

What is still open:

- M5 override governance and debt reduction.
- Final M6 readiness is mostly waiting on the remaining M5 override decision.
- A single open TODO remains: delete or justify overrides that are truly obsolete after typed-model
  or measurement fixes.

## Remaining Work Shape

The remaining work is not another broad pipeline rewrite.
It is mostly evidence-driven debt reduction:

- root viewport buckets that still reflect real `parity-root` drift
- text lookup buckets that still guard real browser/font behavior
- a few retained guards that must stay until the upstream geometry or text model changes

Largest remaining buckets:

- root viewport: `flowchart` 103, `sequence` 79, `gitgraph` 65, `mindmap` 39, `c4` 35,
  `state` 34
- text lookup: `class` 277, `block` 123, `flowchart` 45, `state` 29

## Next Practical Slices

1. Keep shrinking root viewport debt only where typed layout or emitted-bounds logic can absorb it.
2. Keep pruning text lookup debt only where DOM parity, layout snapshots, and strict gates all stay green.
3. Prefer larger structural wins over small one-off deletions when the evidence points to a shared model fix.
4. Re-run the release gate after each meaningful deletion batch.

## Completion Definition

This workstream is finished when:

- `TODO.md` has no unresolved P0 items
- the remaining override debt is either removed or explicitly justified
- `cargo run -p xtask -- verify --strict` is still green
- `cargo bench -p merman --features render` has a fresh release-ready spotcheck
- `CHANGELOG.md` and the audit docs reflect the final state
