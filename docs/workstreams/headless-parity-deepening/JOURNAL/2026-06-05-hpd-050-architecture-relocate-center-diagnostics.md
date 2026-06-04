# HPD-050 - Architecture Relocate Center Diagnostics

Date: 2026-06-05
Task: HPD-050 Architecture-first layout engine audit

## Context

The render-path/source-frame pass narrowed `093` and `002` away from a simple final node movement
or service label-width explanation. Both rows still needed a direct table for the first post-FCoSE
translation step, because Mermaid's Architecture path runs `relocateComponent.before-shift` after
the CoSE/FCoSE run and that step can move the whole component without changing same-stage node
rects.

This pass is diagnostic-only. It changes `debug-architecture-delta` Markdown output and test
coverage, not Architecture layout, rendering, root overrides, baselines, or final SVG output.

## Change

- Added a render-path join table named `Bundled FCoSE/Cose relocate centers vs local trace`.
- The table compares bundled/browser and local `relocateComponent.before-shift` data per run:
  `rectBbox`, `originalCenter`, `rectCenter`, and `delta`.
- The local columns are populated from `layout.fcose_debug_stages`, so real local values require
  `MANATEE_FCOSE_DEBUG_TRACE=1` when regenerating the delta report.
- Updated `architecture_render_path_join_reports_local_deltas` so the fixture covers bundled/local
  relocate input differences.

## Evidence

- `target/compare/architecture-delta-relocate-table-check-trace-hpd050`
- `target/compare/architecture-report-parity-relocate-table-hpd050`
- `crates/xtask/src/cmd/debug/architecture.rs`

Commands:

- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_nested_groups_002 --fixture stress_architecture_batch6_init_fontsize_icon_size_wrap_093 --out target/compare/architecture-delta-relocate-table-check-trace-hpd050 --render-probe-dir target/compare/architecture-render-path-source-frame-002-093-hpd050` with `MANATEE_FCOSE_DEBUG_TRACE=1`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/architecture-report-parity-relocate-table-hpd050`

## Findings

- `002`:
  - run 0 relocate inputs match exactly.
  - run 1 `rectBbox` and `rectCenter` match, but local `originalCenter.x` is `+1.250000px` and
    local `delta.x` is also `+1.250000px`.
  - This supports treating `002` as a source/consumption frame alignment row, not a direct service
    label-width row.
- `093`:
  - run 0 relocate inputs match exactly.
  - run 1 `rectBbox` and `rectCenter` match, but local `originalCenter.x` is `+22.963987px` and
    local `delta.x` is also `+22.963987px`.
  - This confirms a large post-FCoSE component translation difference even when same-stage local
    node rects can align with bundled final-stage data.

## Boundary

Do not use this table to justify a global group padding or root padding change. The table isolates
the relocate center/delta contribution, but residual width still passes through later group bounds
consumption and SVG/root emission phases.

## Verification

- `cargo fmt --check` - passed.
- `cargo nextest run -p xtask architecture_render_path_join_reports_local_deltas` - passed, `1`
  test run.
- `cargo nextest run -p xtask` - passed, `112` tests run.
- `cargo run -p xtask -- report-overrides --check-no-growth` - passed; Architecture root overrides
  remain at `0`.
- `cargo run -p xtask -- debug-architecture-delta ... --out target\compare\architecture-delta-relocate-table-check-trace-hpd050`
  with `MANATEE_FCOSE_DEBUG_TRACE=1` - passed for `002` and `093`.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture-report-parity-relocate-table-hpd050`
  - passed.
