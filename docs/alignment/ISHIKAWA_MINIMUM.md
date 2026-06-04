# Ishikawa Minimum (Mermaid@11.15.0)

This document tracks the first local support slice for Mermaid `ishikawa`.

Upstream references at locked commit `41646dfd43ac83f001b03c70605feb036afae46d`:

- Detector: `packages/mermaid/src/diagrams/ishikawa/ishikawaDetector.ts`
- DB/model: `packages/mermaid/src/diagrams/ishikawa/ishikawaDb.ts`
- Grammar: `packages/mermaid/src/diagrams/ishikawa/parser/ishikawa.jison`
- Renderer: `packages/mermaid/src/diagrams/ishikawa/ishikawaRenderer.ts`
- Styles: `packages/mermaid/src/diagrams/ishikawa/ishikawaStyles.ts`
- Types: `packages/mermaid/src/diagrams/ishikawa/ishikawaTypes.ts`
- Syntax docs: `docs/syntax/ishikawa.md`

## Implemented (Phase 1)

- Detection:
  - accepts `ishikawa` and `ishikawa-beta`
  - exposes internal diagram id `ishikawa`, matching upstream detector id
  - matches upstream case-insensitive header behavior through the detector fallback
- Parser:
  - first text row becomes effect/root and diagram title
  - subsequent rows become causes by indentation
  - first cause indentation is used as the base level, matching upstream `baseLevel`
  - blank lines and whole-line `%%` comments are ignored
- Render model:
  - typed `IshikawaDiagramRenderModel`
  - compatibility JSON from the same typed model
- Layout:
  - ports the upstream spine, alternating cause, side-stat, and sub-branch geometry constants
  - supports `ishikawa.diagramPadding`, `ishikawa.useMaxWidth`, and top-level `fontSize`
- SVG:
  - Stage B renderer with `.ishikawa`, `.ishikawa-spine`, `.ishikawa-branch`, `.ishikawa-sub-branch`, `.ishikawa-head`, `.ishikawa-label-box`, and arrow marker DOM signals
  - uses `themeVariables.lineColor`, `mainBkg`, and `textColor`

## Known Gaps

- `xtask compare-ishikawa-svgs --check-dom --dom-mode parity --dom-decimals 3` passes for the
  current committed baseline corpus.
- A committed upstream SVG baseline corpus exists under `fixtures/upstream-svgs/ishikawa/`.
- Hand-drawn / rough.js mode is not implemented in local SVG output.
- Browser `getBBox()` float parity for labels and head shape has not been strict-audited.
- Full Cypress image snapshot coverage has not been imported.
