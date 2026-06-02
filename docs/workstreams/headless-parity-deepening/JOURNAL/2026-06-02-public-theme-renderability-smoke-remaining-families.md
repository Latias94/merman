# HPD-080 - Public Theme Renderability Smoke Remaining Families

Date: 2026-06-02
Task: HPD-080 visible rendering defect triage

## Context

The previous public renderability smoke expansion covered many supported diagrams but still left a
few implemented families out of the public `HeadlessRenderer` route: ER, Mindmap, C4, Packet, and
Sankey.

Those diagrams already had renderer-level CSS or config tests. This slice checks whether the same
signals survive the public parse/layout/render path that host consumers normally use.

## Outcome

Extended `crates/merman/tests/theme_renderability_smoke.rs` with public dark-theme/config cases for:

- ER
- Mindmap
- C4
- Packet
- Sankey

No production rendering fix was needed.

## Calibration

- Mindmap's `nodeBorder` root-span rule is redux-specific in the local source-backed CSS seam, so
  the smoke sets `theme: "redux"` before asserting that color.
- C4 uses visible C4 config colors in the smoke. This avoids overstating generic `themeVariables`
  support for C4, where Mermaid 11.15's style provider is intentionally narrow and most visible
  palette behavior is C4 config or per-element style.
- Packet and Sankey use diagram config style options because those are the Mermaid 11.15 visible
  style contracts for those diagrams.

## Verification

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render representative_dark_theme_diagrams_keep_visible_theme_signals`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test theme_renderability_smoke`
- `cargo fmt --check -p merman`
- JSONL validation for `CONTEXT.jsonl`, `TASKS.jsonl`, and `CAMPAIGNS.jsonl`
- `git diff --check`

## Residual

Info and Error still have no Mermaid 11.15 diagram-specific style provider. ZenUML remains an
external plugin compatibility boundary. Keep them out of this broad public theme smoke unless a
specific visible failure appears.
