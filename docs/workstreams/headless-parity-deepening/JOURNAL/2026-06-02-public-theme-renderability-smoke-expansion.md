# HPD-080 - Public Theme Renderability Smoke Expansion

Date: 2026-06-02
Task: HPD-080 visible rendering defect triage

## Context

After the Zed host-theme audit, the remaining practical question was whether common custom Mermaid
theme inputs survive through the public headless renderer across the supported matrix, not whether
we can claim browser-exact visual parity.

The existing public smoke covered Flowchart, Sequence, Kanban, GitGraph, QuadrantChart, and
XYChart. This slice expanded that gate to the other supported diagrams with compact visible theme
signals.

## Outcome

Expanded `crates/merman/tests/theme_renderability_smoke.rs` to cover:

- Class
- State
- Architecture
- Block
- Journey
- Radar
- Requirement
- Timeline
- Gantt
- Treemap
- Pie

The smoke still checks semantic renderability only:

- SVG output exists.
- Geometry does not leak `NaN`.
- Unexpected `undefined` tokens are rejected.
- Representative labels are present.
- Source-backed visible theme colors or inline theme settings survive in output.

## Source Checks

Timeline initially failed the `undefined` guard because local output includes
`class="node-bkg node-undefined"`.

That exact class shape is present in pinned Mermaid 11.15 upstream SVG fixtures under
`fixtures/upstream-svgs/timeline`, including stress fixtures. It comes from the same kind of
optional class concatenation as the earlier Kanban/shared-renderer placeholder classes, so it was
narrowly allowed in the smoke scrubber instead of treated as a local rendering defect.

Requirement expected labels were also corrected to match visible renderer output:
`Risk: High` and `Verification: Analysis`, not the lower-case parser tokens.

## Verification

- `rg -n "node-undefined|undefined" fixtures\upstream-svgs\timeline repo-ref\mermaid\packages\mermaid\src\diagrams\timeline -S`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render representative_dark_theme_diagrams_keep_visible_theme_signals`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test theme_renderability_smoke`
- `cargo fmt --check -p merman`
- JSONL validation for `CONTEXT.jsonl`, `TASKS.jsonl`, and `CAMPAIGNS.jsonl`
- `git diff --check`

## Residual

This is not a full theme-parity percentage. It is a public API safety net for visible defects such
as blank output, hidden labels, missing emitted theme colors, invalid geometry, and unexpected
tokens. Future expansion should happen only when a newly supported diagram, source-backed theme
contract, or consumer failure justifies it.
