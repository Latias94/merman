---
type: Verification Evidence
title: "Cynefin compatibility renderer admission"
timestamp: 2026-07-09T15:45:00Z
related_plan: docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md
tags:
  - mermaid-11-16
  - u5
  - cynefin
  - renderer
  - admission
---

# Summary

Cynefin moved from parser-only evidence to a compatibility renderer slice for the 11.16 parity
workstream.

# Evidence

- `cynefin-beta` now has a typed render parser and `RenderSemanticModel::Cynefin`.
- `merman-render` has a source-backed headless layout and SVG renderer for the upstream 11.16
  Cynefin geometry, seeded boundary paths, item badges, confusion overflow, transitions, config,
  and theme variables.
- Admission is `CompatibilityOnly`: semantic and layout fixtures are admitted, while upstream SVG
  baselines and `compare-cynefin-svgs` remain deferred.

# Verification

- `cargo nextest run -p merman-core cynefin --no-fail-fast`
- `cargo nextest run -p merman-render cynefin --no-fail-fast`
- `cargo nextest run -p merman-core registry --no-fail-fast`
- `cargo nextest run -p merman-bindings-core diagram_family_capabilities_expose_detector_parser_and_render_surface --no-fail-fast`
- `cargo nextest run -p xtask admission --no-fail-fast`
- `cargo run -p xtask -- check-alignment`
- `cargo nextest run -p merman-core fixtures_match_golden_snapshots --no-fail-fast`
- `cargo nextest run -p merman-render fixtures_match_layout_golden_snapshots_when_present --no-fail-fast`
- `cargo fmt --check`
- `git diff --check`

# Residual

Cynefin still lacks upstream SVG baselines and `compare-cynefin-svgs`. The Cynefin layout golden was
generated with `cargo run -p xtask -- update-layout-snapshots --diagram cynefin`.
