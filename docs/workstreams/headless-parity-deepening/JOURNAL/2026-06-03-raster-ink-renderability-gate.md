# HPD-080 Raster Ink Renderability Gate

Date: 2026-06-03

## Problem

The `resvg_safe` fixture smoke proved that SVG output could be parsed, sanitized, and converted to
PNG, but a blank or all-background PNG could still pass. That leaves a real host-facing failure
class uncovered: diagrams shifted out of the viewport, fully transparent output, or a raster image
with only the root background.

## Change

The raster branch of `crates/merman/tests/resvg_safe_fixture_smoke.rs` now decodes the PNG with the
`png` crate and checks for visible pixels that differ from the first-pixel background.

The check is source-aware. It only requires non-background ink when the source contains real diagram
content. Header-only, accessibility-only, and title-only metadata fixtures still have to emit valid
resvg-safe SVG and rasterize successfully, but they are not treated as blank-rendering failures.

This calibration matters for Architecture: Mermaid 11.15 parses `architecture-beta title ...`, but
the pinned upstream SVG does not render a visible title, and `architectureRenderer.ts` still marks
Architecture title support as TODO.

## Verification

- `cargo fmt --check -p merman`
- `cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke`
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='architecture,class,sequence'; cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit`

## Residual Notes

This is not pixel parity and should not become pixel parity. It is a gross renderability guard for
contentful diagrams. Fine color differences, antialiasing, text metrics, and unsupported title
semantics still need source-backed focused evidence.
