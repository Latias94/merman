---
type: "Memory Event"
title: "Verification: U4 TreeView 11.16 parser/render/LSP slice"
description: "TreeView now supports 11.16 bare labels, directory nodeType, class/icon/description annotations, box drawing input, original-source editor spans, and basic icon/description/highlight SVG DOM."
timestamp: 2026-07-09T14:32:04Z
event_kind: "Verification"
---
# Event

U4 TreeView 11.16 slice complete: the parser moved from quoted-only nodes to a line scanner that
matches Mermaid 11.16 TreeView semantics for bare labels, quoted labels, trailing-slash directories,
`:::class`, `icon(...)`, `icon(none)`, empty `icon()`, `## description`, and box-drawing tree input.
Editor facts parse original source rather than preprocessed text so box-drawing node and annotation
spans remain LSP-usable. The typed render model and semantic JSON now include `nodeType`, optional
`cssClass`, optional `icon`, and optional `description`.

# Impact

TreeView semantic/layout/SVG goldens must be refreshed from the 11.16 baseline after this commit,
because model JSON and rendered DOM intentionally gained 11.16 fields and elements. The TreeView
parser stays hand-written; LALRPOP would add little value for indentation/box-drawing remap while
making editor span recovery worse.

# Verification

- `cargo nextest run -p merman-core tree_view --no-fail-fast`
- `cargo nextest run -p merman-render tree_view --no-fail-fast`
- `cargo check -p merman-ascii --tests`
- `cargo fmt --check`
- `cargo run -p xtask -- update-snapshots --diagram treeView`
- `cargo run -p xtask -- update-layout-snapshots --diagram treeView`
- Attempted `cargo run -p xtask -- gen-upstream-svgs --diagram treeView`; blocked because
  Puppeteer could not find Chrome `131.0.6778.204` in `C:\Users\Frankorz\.cache\puppeteer`.
  Upstream SVG baseline refresh remains pending for U7 after browser installation/cache repair.

# Citations

- `repo-ref/mermaid/packages/mermaid/src/diagrams/treeView/parser.ts`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/treeView/boxDrawingPreprocessor.ts`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/treeView/icons.ts`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/treeView/renderer.ts`
