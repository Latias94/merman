# Ishikawa Minimum (Mermaid@11.16.0)

This document tracks the first local support slice for Mermaid `ishikawa`.

Upstream references at locked commit `7c0cafcf42e76bfaf79d0cbbd12edb986612f014`:

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
  - preserves the upstream `spine -> pair -> branch -> label/sub-group` ownership as typed layout
    data instead of flattening lines, labels, and label boxes into unrelated arrays
  - supports `ishikawa.diagramPadding`, `ishikawa.useMaxWidth`, and top-level `fontSize`
- SVG:
  - Stage B renderer with source-backed `.ishikawa-pair`, `.ishikawa-label-group`, and
    `.ishikawa-sub-group` ownership in addition to the spine, branch, head, label-box, and arrow
    marker DOM signals
  - uses `themeVariables.lineColor`, `mainBkg`, and `textColor`

## Known Gaps

- Both `structure` and `parity` DOM modes pass all 12 fixtures in the current committed baseline
  corpus. The former 11-fixture wrapper residual was closed by retaining the renderer's typed group
  ownership; no comparator normalization or fixture-specific policy is involved.
- A committed upstream SVG baseline corpus exists under `fixtures/upstream-svgs/ishikawa/`.
- Hand-drawn / rough.js mode is not implemented in local SVG output.
- `look: "handDrawn"` remains a dedicated follow-up lane and should not be promoted in the
  config support matrix until Ishikawa has rendered SVG tests proving deterministic RoughJS output
  and seed behavior for the spine, branches, head, and label boxes.
- Browser `getBBox()` float parity for labels and head shape has not been strict-audited.
- Full Cypress image snapshot coverage has not been imported.
