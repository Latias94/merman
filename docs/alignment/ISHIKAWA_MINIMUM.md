# Ishikawa Minimum (Mermaid@11.16.1)

This document tracks the first local support slice for Mermaid `ishikawa`.

Upstream references at locked commit `7ecca0cd7f1658ef74f4e7e91f925724ef403bbf`:

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
  - `look: "handDrawn"` ports the Mermaid 11.16 RoughJS line, path, and rectangle branches for the
    head, spine, branches, label boxes, hachure fills, and solid arrows while preserving upstream
    DOM order
  - a fixed `handDrawnSeed` produces deterministic local rough paths; different seeds produce
    different geometry
  - uses `themeVariables.lineColor`, `mainBkg`, and `textColor`

## Known Gaps

- `structure` DOM mode passes all 13 fixtures in the current baseline corpus. The 12 classic-look
  fixtures also pass `parity` mode. The former wrapper residual was closed by retaining the
  renderer's typed group ownership; no comparator normalization or fixture-specific policy is
  involved.
- A committed upstream SVG baseline corpus exists under `fixtures/upstream-svgs/ishikawa/`.
- Cypress case 6 is pinned with `handDrawnSeed: 1` for reproducible baseline generation. Upstream's
  default seed `0` delegates to random RoughJS seeding and does not produce a stable SVG across
  independent renders; the explicit seed is a deterministic baseline-policy adaptation.
- The hand-drawn fixture retains path-coordinate differences between JavaScript RoughJS and the
  Rust `roughr` implementation. Its group/path structure and paint attributes converge, but exact
  RoughJS path geometry remains a documented parity residual rather than comparator normalization.
- Browser `getBBox()` float parity for labels and head shape has not been strict-audited.
- All 12 Cypress rendering inputs are represented. Browser image-pixel snapshots are not part of
  the headless DOM comparison contract.
