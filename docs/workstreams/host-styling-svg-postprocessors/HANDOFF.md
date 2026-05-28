# Handoff

Workstream: Host Styling SVG Postprocessors
Status: Complete
Last updated: 2026-05-28

## Current State

The lane is implemented and verified. `SvgPipeline` now lives under a focused `pipeline/` module
tree, postprocessors receive diagram metadata, and product-neutral styling blocks are available for
scoped CSS injection and opt-in CSS override behavior. Readable fallback text also preserves useful
class, fill, and font context for host CSS.

## Next Action

No required follow-up remains for this lane.

Future work, if needed, should be split into a separate lane:

- structured XML/CSS parsing for host styling built-ins;
- before/after preset insertion points for advanced hosts;
- additional product-neutral styling helpers discovered from real downstream integrations.

## Guardrails

- Do not change `render_svg_sync` defaults.
- Do not add Zed-specific accent/theme semantics to core.
- Keep `SvgPostprocessor` source-compatible.
- Record fresh evidence before closing tasks.

## Closeout Evidence

- `cargo nextest run -p merman-render` passed: 220 tests.
- `cargo nextest run -p merman --features raster` passed: 15 tests.
- `cargo nextest run -p merman-cli` passed: 8 tests.
- Clippy, fmt check, and `git diff --check` passed.
