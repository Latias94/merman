# HPD-050 - Mindmap Deep-Tree Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

After Treemap's unbounded hierarchy path was converted away from recursive traversal, the next
tree-shaped family audit targeted Mindmap. Mindmap also accepts deeply nested user-authored
hierarchies without a custom depth rejection boundary, so public parse/layout paths should not
depend on the Rust call stack.

## Changes

- Replaced recursive Mindmap section assignment with explicit stack traversal.
- Replaced recursive semantic flat node and edge projection with explicit heap-backed traversal.
- Replaced recursive typed render node and edge projection with explicit heap-backed traversal.
- Replaced recursive nested `rootNode` JSON projection with explicit postorder traversal that moves
  child `serde_json::Value`s upward.
- Replaced the final non-empty Mindmap semantic object assembly with a hand-built
  `serde_json::Map`, avoiding deep `json!` wrapping of the nested `rootNode`.
- Updated Mindmap's semantic-JSON layout entrypoint to deserialize only the flat `nodes` / `edges`
  fields used by layout, avoiding recursive serde traversal of the deep `rootNode` compatibility
  field.
- Added regressions through public paths:
  - core parses and semantically projects a `1,200`-level Mindmap hierarchy;
  - core builds the typed render model for the same hierarchy;
  - render parses the ordinary JSON semantic path and layouts the same hierarchy.

## Verification

- `cargo fmt --check -p merman-core -p merman-render` - passed.
- `cargo nextest run -p merman-core mindmap` - passed, `34` tests run.
- `cargo nextest run -p merman-render --test mindmap_svg_test` - passed, `4` tests run.
- `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed.
- `git diff --check` - passed.

## Notes

- No SVG baseline, root override, Architecture root-bounds formula, or Mermaid parity fixture was
  changed.
- This slice hardens Mindmap's accepted deep hierarchy path. It does not introduce a new Mindmap
  depth limit.
