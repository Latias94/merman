# Cynefin Minimum (Mermaid@11.16.0)

This document tracks the first local support slice for Mermaid `cynefin-beta`.

Upstream references at pinned Mermaid 11.16.0:

- Detector: `packages/mermaid/src/diagrams/cynefin/detector.ts`
- Parser adapter: `packages/mermaid/src/diagrams/cynefin/parser.ts`
- DB/model: `packages/mermaid/src/diagrams/cynefin/db.ts`
- Renderer: `packages/mermaid/src/diagrams/cynefin/cynefinRenderer.ts`
- Boundary helpers: `packages/mermaid/src/diagrams/cynefin/cynefinBoundaries.ts`
- Styles: `packages/mermaid/src/diagrams/cynefin/styles.ts`
- Types: `packages/mermaid/src/diagrams/cynefin/types.ts`

## Implemented

- Detection:
  - accepts `cynefin-beta`
  - exposes internal diagram id `cynefin`
- Parser:
  - common `title`, `accTitle`, and `accDescr`
  - five upstream domains: `complex`, `complicated`, `clear`, `chaotic`, and `confusion`
  - quoted item labels with single or double quotes
  - domain transitions with optional quoted labels
  - duplicate domain blocks replace earlier items, matching Mermaid's map-style DB behavior
  - self-loop transitions are skipped and surfaced to editor diagnostics
- LSP/editor facts:
  - header, common directives, domain symbols, item labels, transition endpoints, and transition labels
  - recoverable diagnostics for malformed lines while preserving source spans
- Render model:
  - typed `CynefinDiagramRenderModel`
  - compatibility JSON from the same typed model
- Layout:
  - upstream fixed domain geometry for the four quadrants and center confusion ellipse
  - `cynefin.width`, `height`, `padding`, `showDomainDescriptions`, `boundaryAmplitude`, `seed`, and `useMaxWidth`
  - deterministic item badge measurement through the existing headless `TextMeasurer`
  - confusion overflow badge after three rendered items
  - transition quadratic control point geometry between domain centers
- SVG:
  - domain backgrounds, wavy boundaries, cliff, confusion ellipse, labels, subtitles, item badges, transitions, and visible title
  - Cynefin-specific theme variables from `themeVariables.cynefin`
  - accessibility DOM for `accTitle` and `accDescr`

## Admission State

`cynefin` is recorded as `CompatibilityOnly` in the admission inventory:

- semantic JSON fixtures are normalized under `fixtures/cynefin/`
- layout goldens are normalized under `fixtures/cynefin/`
- local SVG rendering is implemented
- upstream SVG baselines and a family-local `compare-cynefin-svgs` command are deferred to the U7 Mermaid 11.16 baseline refresh

## Known Gaps

- No committed Cynefin upstream SVG corpus yet.
- No dedicated `xtask compare-cynefin-svgs` command yet.
- Browser `getBBox()` item width is approximated through the repository's deterministic text measurement path.
- Boundary path number formatting is normalized for stable headless output rather than forced to exact browser stringification.
