# HPD-050 - TreeView Depth-Boundary Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

After the Ishikawa deep-tree cleanup removed the most obvious unbounded recursive tree walkers, the
next tree-shaped family audit targeted `treeView-beta`. Unlike Ishikawa, TreeView already enforces
`MAX_DIAGRAM_NESTING_DEPTH`, so the release-boundary target was the maximum accepted parse/layout
path rather than accepting arbitrarily deep user input.

## Changes

- Replaced recursive TreeView arena-to-render-model conversion with an explicit postorder stack.
- Replaced recursive semantic `nodes` flattening with an explicit stack while preserving preorder
  output.
- Replaced recursive root JSON projection through serde with an explicit postorder stack that
  preserves the nested `root` shape.
- Replaced render-side recursive layout with an explicit enter/exit stack, preserving preorder node
  rows and postorder vertical-line emission.
- Added public-path regressions:
  - core parses and semantically projects the maximum accepted `256`-node TreeView chain;
  - render parses and layouts the same maximum accepted chain.

## Verification

- `cargo fmt --check -p merman-core -p merman-render` - passed.
- `cargo nextest run -p merman-core tree_view` - passed, `5` tests run.
- `cargo nextest run -p merman-render --test tree_view_svg_test` - passed, `5` tests run.
- `cargo run -p xtask -- compare-tree-view-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed.
- `git diff --check` - passed.

## Notes

- The TreeView depth limit is unchanged. Inputs beyond `MAX_DIAGRAM_NESTING_DEPTH` still return the
  existing parse/model error.
- No SVG baseline, root override, Architecture root-bounds formula, or Mermaid parity fixture was
  changed.
