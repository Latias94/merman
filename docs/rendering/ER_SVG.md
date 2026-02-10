# ER Diagram SVG (Stage B, WIP)

This document describes the current `merman-render` ER Diagram SVG output.

Baseline: Mermaid `@11.12.2`.

Status: **work in progress** (structure-first, then style parity).

## Run (single file)

PowerShell:

`Get-Content fixtures\\er\\basic.mmd | cargo run -p merman-render --example er_svg > out.svg`

## Bulk export (all fixtures)

`cargo run -p xtask -- gen-er-svgs`

Outputs to: `target/svgs/er/*.svg`

## What It Produces Today

- Entity boxes with title and attribute rows (column layout follows the Mermaid ER renderer logic used by Mermaid 11.12.2).
- Relationship lines (with ER cardinality markers via `marker-start`/`marker-end`).
- Relationship paths use a port of D3 `curveBasis` (Mermaid-like cubic beziers), driven by Dagre-style
  edge points (including node intersection endpoints).
- Dashed relationship lines for `NON_IDENTIFYING` relationships (`stroke-dasharray: 8,8`).
- Relationship label text with an opaque label box behind it.
- Diagram title via YAML front-matter `title:` (centered, above the diagram), with viewBox/size including the title area.
- Mermaid-like viewport sizing (padding + `useMaxWidth` handling).

## Known Gaps (Parity Roadmap)

- Theme variables and styling parity (colors, fonts, precise CSS selectors).
- Full support for Mermaid `style` / `classDef` / `class` application semantics (currently applied for entity boxes + entity/attribute text; relationship labels still use defaults).
