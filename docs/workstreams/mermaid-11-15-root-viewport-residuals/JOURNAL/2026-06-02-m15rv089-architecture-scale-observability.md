# M15RV-089 - Architecture Scale Observability

Date: 2026-06-02
Task: M15RV-089

## Summary

Exposed the applied Architecture canvas-label width scale as explicit metrics/debug data so the
remaining long-label residuals can be reasoned about without more guesswork.

## Why

The current Architecture headless approximation already has a committed piecewise rule for long
labels (`>= 200px -> 1.01`). That rule improved the targeted long-title row, but it is still easy
to deceive ourselves about whether later changes are helping because the scale choice was hidden
inside a helper.

Before changing the rule again, it is better to make the chosen branch visible and verify that
observability changes do not perturb the focused residuals.

## Scope

- `ArchitectureCytoscapeCanvasLabelMetrics` now carries `applied_scale`.
- The Architecture debug trace now prints that scale.
- Added a focused unit test for the new metrics seam.

## Verification

- `cargo test -p merman-render architecture_text_constants_match_mermaid -- --nocapture`
- `cargo test -p merman-render architecture_canvas_label_metrics_report_applied_scale -- --nocapture`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch5_long_titles_and_punct_076 --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/architecture_batch5_long_titles_probe_after_scale_observability.md`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch4_init_small_icons_061 --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/architecture_batch4_small_icons_probe_after_scale_observability.md`

## Notes

This is a seam-strengthening change, not a residual-count claim. The two focused Architecture rows
stayed exactly where they were:

- `stress_architecture_batch5_long_titles_and_punct_076`: `+5px`
- `stress_architecture_batch4_init_small_icons_061`: `-9.25px`

That is the useful result here: the approximation layer is easier to inspect, and there was no
silent geometry drift while doing it.
