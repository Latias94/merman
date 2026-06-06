# HPD-050 - Ishikawa Deep-Tree Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

After the Architecture top-service icon/root-bounds audit classified the remaining small icon rows
as bounded diagnostics, the next useful release-boundary slice moved to production panic surfaces.
The target was user-authored Ishikawa cause/subcause trees, which previously had several recursive
tree walkers in public parse/render paths.

## Changes

- Replaced recursive Ishikawa arena-to-render-model conversion with an explicit postorder stack.
- Replaced recursive semantic `nodes` flattening with an explicit stack while preserving depth-first
  output order.
- Replaced recursive root JSON projection with an explicit postorder stack instead of relying on
  nested `json!` serialization.
- Replaced render-side descendant counting and render label-entry flattening with explicit stacks.
- Changed the odd-depth parent-bone lookup from `expect("parent bone exists")` to a branch-local
  fallback bone if the traversal invariant is ever violated.
- Added regressions through public paths:
  - core parses and semantically projects a `1,500`-level Ishikawa hierarchy;
  - render parses and layouts a `1,200`-level Ishikawa hierarchy.

## Verification

- `cargo fmt --check -p merman-core -p merman-render` - passed.
- `cargo nextest run -p merman-core ishikawa` - passed, `5` tests run.
- `cargo nextest run -p merman-render --test ishikawa_svg_test` - passed, `2` tests run.
- `git diff --check` - passed.

## Notes

- No SVG baseline, root override, Architecture root-bounds formula, or Mermaid parity fixture was
  changed.
- This slice hardens one concrete parser/render tree boundary. It does not claim all recursive
  tree-shaped families are fully audited.
