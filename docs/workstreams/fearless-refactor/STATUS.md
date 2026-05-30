# Fearless Refactor Status

Snapshot: 2026-05-30

This page is the short-form dashboard for the fearless-refactor workstream.
The detailed plan still lives in `TODO.md`, `MILESTONES.md`, `OVERRIDE_FOOTPRINT.md`, and `COMPLETION_AUDIT.md`.

## Current Read

Current-release closeout: complete.

Future derivation backlog remains, but no current-release P0 closeout item is open.

What is done:

- M0 safety baseline is complete.
- M1 pipeline ownership cleanup is complete.
- M2 typed model expansion is complete for all in-tree Mermaid diagrams except the explicit error/custom-registry fallback path.
- M3 text subsystem modularization is complete.
- M4 large renderer decomposition is effectively complete.
- Render numeric config parsing is centralized in `crates/merman-render/src/config.rs`; diagram
  modules no longer carry local `json_f64` / `config_f64` / CSS `px` parser copies.
- Root viewport override no-growth is now `286` according to
  `cargo run -p xtask -- report-overrides --check-no-growth`. The root-viewport derivation
  workstream removed additional generated pins, most recently replacing the ER
  `DELIVERY-ADDRESS`, `PRODUCT-CATEGORY`, `Customer Account Tertiary`, `CATEGORY`,
  `This **is** _Markdown_`, and `ATLAS-TEAMS` root buckets with
  ER-owned browser label-width facts, and now governs the nine current full-strict outside-table
  root residuals with an exact
  `compare-all-svgs` policy instead of silent debt.
- Sequence layout has been split down to focused actor, activation, block-step, block-bounds,
  note, message, rect, root-bounds, and orchestration owners.
- `cargo run -p xtask -- verify --strict` passes; the latest closeout run covered workspace
  nextest (`1246` passed, `3` skipped), normal SVG DOM parity, and full root parity with the
  explicit nine-residual policy.
- `cargo run -p xtask -- verify --strict` includes full `parity-root` coverage.
- `cargo run -p xtask -- report-overrides --check-no-growth` passes.
- A disabled-root cross-check after numeric config parser centralization found no newly stale root
  viewport pins across generated root tables, so current root debt remains retained evidence rather
  than cleanup-by-count work.
- The latest root closeout keeps generated root pins stale-free and locks the nine accepted
  outside-table root residuals to exact fixture/value pairs, so changed or additional residuals
  fail the strict gate.
- `cargo bench -p merman --features render` has a fresh post-cleanup release gate record in
  `docs/performance/spotcheck_2026-05-14_flowchart_override_inventory_full_bench_gate.md`.
- Root `CHANGELOG.md` now calls out the refactor release-readiness work.
- Clippy is part of the strict release gate.
- Hand-curated helper overrides are at `0`.
- Manual raw SVG/path bridge functions are at `0`.

What is still open:

- Future root viewport and text lookup derivation targets remain for later releases.
- No known-obsolete override bucket is waiting on blind deletion in the current release scope.
- Any future override reduction should still start from disabled-root or text-measurement evidence,
  not table pruning by count alone.

## Remaining Work Shape

The remaining work is not another broad pipeline rewrite.
It is mostly evidence-driven debt reduction:

- root viewport buckets that still reflect real `parity-root` drift
- text lookup buckets that still guard real browser/font behavior
- explicit browser/font buckets whose exactness cost would exceed the value of a cleaner model
- a few retained guards that must stay until the upstream geometry or text model changes

Largest remaining buckets:

- root viewport: `sequence` 58, `flowchart` 43 inventory entries / 49 fixture keys, `mindmap` 39,
  `c4` 35, `state` 33, `architecture` 31, `gitgraph` 23, `requirement` 7, `er` 6,
  `timeline` 8, `sankey` 3
- text lookup: `class` 277, `block` 123, `flowchart` 45, `state` 29, `er` 9

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
