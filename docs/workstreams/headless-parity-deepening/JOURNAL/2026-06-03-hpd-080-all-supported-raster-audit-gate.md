# HPD-080 - All-Supported Raster Audit Gate Calibration

Date: 2026-06-03

## Context

After the C4 headless-shell measurement slice, the next HPD-080 step was to resume broad
renderability scanning before returning to numeric root residuals. The raster-enabled
`all_supported_fixtures_render_headless_resvg_safe_audit` gate initially stopped on
`fixtures/treemap/upstream_treemap_classdef_and_css_compiled_styles_db.mmd` because strict public
rendering correctly rejects the bare `classDef ... color;` token.

That fixture already has a `diagramType: "error"` golden, and the pinned source evidence in the
earlier Pie/Treemap slice says the parser/render result is the parity contract. The audit gate was
wrong to treat it as a contentful Treemap renderability sample.

## Outcome

- Added the two Treemap classDef bare-token error-golden fixtures to the manual audit skip list.
- Kept Treemap production parsing strict; this slice does not relax parser behavior.
- Added a focused test proving those known error-golden fixtures are skipped by the manual audit.
- Re-ran the supported-family raster audit in filtered batches so failures remain attributable to a
  diagram family or Flowchart corpus group.
- No new production visible rendering defect was found in this pass. Supported-family filtered
  raster audits passed after the Treemap error-golden gate calibration.

## Touched Surfaces

- `crates/merman/tests/resvg_safe_fixture_smoke.rs`

## Verification

- `cargo fmt --check` - passed.
- `cargo nextest run -p merman --features render,raster known_error_golden_fixtures_are_skipped_by_manual_audit source_content_gate_distinguishes_accessibility_only_from_visible_content` -
  passed, `2` tests run.
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='timeline,journey,requirement,gantt,treemap'; cargo nextest run -p merman --features render,raster --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit` -
  passed.
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='c4,packet,pie,quadrantchart,radar,sankey,xychart'; cargo nextest run -p merman --features render,raster --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit` -
  passed.
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='block,er,kanban,mindmap'; cargo nextest run -p merman --features render,raster --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit` -
  passed.
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='architecture'; cargo nextest run -p merman --features render,raster --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit` -
  passed.
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='class'; cargo nextest run -p merman --features render,raster --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit` -
  passed.
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='sequence'; cargo nextest run -p merman --features render,raster --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit` -
  passed.
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='state'; cargo nextest run -p merman --features render,raster --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit` -
  passed.
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='gitgraph'; cargo nextest run -p merman --features render,raster --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit` -
  passed.
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='flowchart'; cargo nextest run -p merman --features render,raster --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit` -
  passed.

## Residual Boundary

This slice only calibrates the manual renderability audit and records a broad raster scan. It does
not close HPD-080. Future HPD-080 work should now require either a failing renderability gate, a
source-backed emitted-surface gap, or concrete consumer evidence before adding more visible-theme
or raster-safety code.
