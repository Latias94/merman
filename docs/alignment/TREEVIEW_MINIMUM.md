# TreeView Minimum (Mermaid@11.15.0)

This document tracks the first local support slice for Mermaid `treeView`.

Upstream references at locked commit `41646dfd43ac83f001b03c70605feb036afae46d`:

- Detector: `packages/mermaid/src/diagrams/treeView/detector.ts`
- Parser adapter: `packages/mermaid/src/diagrams/treeView/parser.ts`
- DB/model: `packages/mermaid/src/diagrams/treeView/db.ts`
- Renderer: `packages/mermaid/src/diagrams/treeView/renderer.ts`
- Styles: `packages/mermaid/src/diagrams/treeView/styles.ts`
- Grammar: `packages/parser/src/language/treeView/treeView.langium`
- Syntax docs: `docs/syntax/treeView.md`

## Implemented (Phase 1)

- Detection:
  - accepts `treeView-beta`
  - exposes internal diagram id `treeView`, matching upstream detector id
- Parser:
  - quoted node names with either `'single'` or `"double"` quotes
  - indentation-count hierarchy using spaces/tabs as one character each
  - virtual root `/` with id `0` and level `-1`
  - common `title`, `accTitle`, and `accDescr` before node rows
  - `%%` line/comment stripping outside quoted node names
- Render model:
  - typed `TreeViewDiagramRenderModel`
  - compatibility JSON from the same typed model
- Layout:
  - preorder recursive row layout
  - upstream defaults: `rowIndent=10`, `paddingX=5`, `paddingY=5`, `lineThickness=1`
  - config overrides from `treeView.*`
- SVG:
  - Stage B renderer with `.tree-view`, `.treeView-node-label`, and `.treeView-node-line`
  - `themeVariables.treeView.labelFontSize`, `labelColor`, and `lineColor`
  - upstream viewBox left offset rule `-lineThickness / 2`

## Known Gaps

- No dedicated `xtask compare-tree-view-svgs` command yet.
- No committed upstream SVG baseline corpus yet.
- Parser diagnostics are Rust-native and not exact Langium error strings.
- SVG DOM ordering and browser `getBBox()` float details have not been strict-parity audited.
- Title/accessibility DOM output is not yet audited against upstream treeView renderer behavior.
