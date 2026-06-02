# HPD-080 Resvg-Safe Fixture Smoke

Date: 2026-06-02
Task: HPD-080

## Context

Zed integration feedback showed that host-facing output quality can fail even when DOM structural
parity looks healthy. The most important host boundary is not exact browser palette policy; it is
whether merman's public headless render path emits SVG that is readable, rasterizer-safe, and still
carries source-backed theme signals.

## Work

- Added `crates/merman/tests/resvg_safe_fixture_smoke.rs`.
- The smoke covers the two user-reported examples:
  - Kanban cards with Mermaid 11.15 metadata attributes.
  - GitGraph branch/merge flow with `develop` and `feature`.
- The smoke also covers a dark themed Flowchart case to prove theme colors survive the
  `HeadlessRenderer::render_svg_resvg_safe_sync(...)` path.
- Added deterministic fixture sampling across supported diagram families. The sample includes each
  family's `basic.mmd` when present, Zed PR 57644 fixtures, and a small sorted set of representative
  stress/upstream fixtures.
- The assertions are deliberately functional:
  - output is SVG and XML-parseable,
  - no `<foreignObject>` remains,
  - unsupported raster CSS constructs are stripped,
  - invalid visual values such as `NaN`, `Infinity`, and `fill="undefined"` are rejected,
  - empty style elements are rejected,
  - when the `raster` feature is enabled, the same SVG must convert to non-empty PNG bytes.

## Verification

- `cargo fmt --check -p merman`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test resvg_safe_fixture_smoke`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke`

## Outcome

No new production renderer defect was found in this slice. The user-provided Kanban and GitGraph
examples, the dark-theme Flowchart sample, and the representative supported-family fixture sample
all render through the resvg-safe public API and rasterize under the `raster` feature.

## Residual Notes

This is not a full fixture pass-rate metric and should not be reported as all-fixture parity. It is
a host-integration regression gate for obvious visible/raster failures. Future HPD-080 work should
continue widening it only when the added cases represent concrete supported families or real host
feedback, not to manufacture a precise-looking parity percentage.
