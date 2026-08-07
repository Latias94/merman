# Wardley Minimum (Mermaid@11.16.1)

This document records the admitted headless implementation of Mermaid `wardley-beta`.

Upstream references at pinned Mermaid 11.16.1:

- Detector: `packages/mermaid/src/diagrams/wardley/wardleyDetector.ts`
- Parser adapter: `packages/mermaid/src/diagrams/wardley/wardleyParser.ts`
- Builder and DB: `packages/mermaid/src/diagrams/wardley/wardleyBuilder.ts` and `wardleyDb.ts`
- Renderer: `packages/mermaid/src/diagrams/wardley/wardleyRenderer.ts`
- Styles: `packages/mermaid/src/diagrams/wardley/styles.ts`

## Implemented

- Detection and parsing:
  - accepts the `wardley-beta` header and exposes the internal family id `wardley`
  - ports common title and accessibility directives, canvas size, axes, custom evolution stages,
    anchors, components, pipelines, links, trends, annotations, notes, accelerators, and
    deaccelerators
  - preserves Mermaid builder ordering, duplicate-node merge behavior, endpoint resolution,
    pipeline membership, source strategies, inertia, labels, and warnings
- Semantic and editor ownership:
  - constructs one span-rich family semantic source
  - projects compatibility JSON, the typed render model, and LSP/editor facts from that source
  - exposes exact statement, symbol, reference, and diagnostic spans without body text scanning
- Typed layout:
  - ports projection, axes, equal or custom stage boundaries, grid, pipeline boxes and evolution
    links, clipped component links, flow markers, trends, source overlays, annotations, and arrows
  - routes annotation text measurement through the operation-owned text measurer
  - retains Mermaid's source-defined `+105px` annotation-box buffer as pinned renderer behavior
  - rejects coincident link endpoints instead of serializing browser `NaN` geometry
- SVG:
  - serializes only the typed Wardley geometry
  - preserves Mermaid's layer order, including separate pipeline and pipeline-link groups and
    root-level marker definitions after the map group
  - resolves all Wardley theme roles directly from `themeVariables.wardley`, with the same upstream
    fallback chain and no CSS post-pass
  - emits unified root viewport and accessibility metadata

## Admission State

`wardley` is admitted to the primary SVG parity matrix:

- normalized semantic and layout fixtures live under `fixtures/wardley/`
- the corpus contains ten source-exact Mermaid 11.16 Cypress cases, including the four standard
  themes, custom canvas size, pipelines, all link forms, annotations, and the GPT tokeniser map
- upstream SVGs are provenance-locked by per-input and per-output hashes in a schema-v2 manifest
- `compare-wardley-svgs --check-dom --dom-mode parity` exercises the canonical typed operation

## Residual Boundary

Annotation-box width and height originate in browser `getComputedTextLength()` and `getBBox()`.
The headless renderer uses the configured text-measurement backend for those two measurements and
otherwise keeps the upstream algorithm unchanged. Font rasterization floats remain a bounded
browser residual; they are not hidden by broad comparator normalization or fixture-specific
overrides.
