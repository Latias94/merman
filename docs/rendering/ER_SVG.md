# ER Diagram SVG (Stage B, WIP)

This document describes the current `merman-render` ER Diagram SVG output.

Baseline: Mermaid `@11.12.2`.

Status: **work in progress** (structure-first, then style parity).

## Run (single file)

PowerShell:

`Get-Content fixtures\\er\\basic.mmd | cargo run -p merman-render --example er_svg > out.svg`

## What It Produces Today

- Entity boxes with title and attribute rows (column layout follows the legacy Mermaid ER renderer logic).
- Relationship lines (with ER cardinality markers via `marker-start`/`marker-end`).
- Dashed relationship lines for `NON_IDENTIFYING` relationships (`stroke-dasharray: 8,8`).
- Relationship label text with an opaque label box behind it.

## Known Gaps (Parity Roadmap)

- Theme variables and styling parity (colors, fonts, precise CSS selectors).
- Exact edge curve rendering (Mermaid uses a D3 basis curve; implemented via cubic Béziers).
- Full support for Mermaid `style` / `classDef` / `class` application semantics (we currently apply `cssStyles` only to the entity box).
