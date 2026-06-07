# HPD-050 - Treemap Deep-Tree Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

After TreeView's accepted depth boundary was converted away from recursive traversal, the next
tree-shaped family audit targeted Treemap. Treemap differs from TreeView because it has no custom
nesting-depth rejection path; deeply nested hierarchy input is accepted and should not depend on the
Rust call stack.

## Changes

- Replaced recursive Treemap semantic `root` projection with explicit postorder traversal.
- Replaced recursive typed render-model construction with explicit postorder traversal.
- Replaced recursive flat semantic `nodes` projection with explicit preorder traversal.
- Replaced deep `json!` semantic output assembly with hand-built `serde_json::Map` output so deep
  `Value` trees are moved into the result instead of recursively serialized.
- Replaced render-side recursive typed-model flattening, subtree sum computation, and child sorting
  with explicit stacks.
- Replaced Treemap semantic-JSON layout deserialization with an iterative semantic projection.
- Added a shared non-recursive `serde_json::Value` clone for `layout_parsed(...)`, after the
  Treemap deep-chain regression reproduced stack overflow in the retained semantic clone path.
- Added regressions through public paths:
  - core parses and semantically projects a `1,200`-level Treemap hierarchy;
  - core builds the typed render model for the same hierarchy;
  - render parses the ordinary JSON semantic path and layouts the same hierarchy.

## Verification

- `cargo fmt --check -p merman-core -p merman-render` - passed.
- `cargo nextest run -p merman-core treemap` - passed, `13` tests run.
- `cargo nextest run -p merman-render --test treemap_svg_test` - passed, `6` tests run.
- `cargo run -p xtask -- compare-treemap-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed.
- `git diff --check` - passed.

## Notes

- No SVG baseline, root override, Architecture root-bounds formula, or Mermaid parity fixture was
  changed.
- This slice hardens Treemap's accepted deep hierarchy path. It does not introduce a new Treemap
  depth limit.
