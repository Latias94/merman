# HPD-050 - Block Deep-Composite Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

After Mindmap's unbounded hierarchy path was converted away from recursive traversal, the next
panic-surface audit targeted Block. Block already used explicit stacks for several tree walkers,
but a `1,200`-level nested composite input still reproduced stack overflow through public core and
SVG rendering paths.

## Changes

- Replaced recursive derived `Block` tree cloning during Block DB parent-child population with an
  explicit postorder clone helper.
- Changed `BlockDb::blocks_flat(...)` to return references instead of recursively cloning stored
  subtrees before semantic and typed render projection.
- Kept Block semantic and typed render projection heap-backed, and changed completed-child map
  lookups from `expect(...)` to conservative missing-child degradation.
- Replaced final `parse_block(...)` semantic object assembly with a hand-built `serde_json::Map`,
  avoiding deep `json!` wrapping of nested block values.
- Added a non-recursive Block semantic `Value` to typed render-model projection in
  `merman-render`, so layout and SVG entrypoints no longer recursively deserialize the deep
  `blocksFlat` tree.
- Replaced SVG-side recursive `collect_nodes(...)` metadata collection with an explicit stack.
- Added regressions through public paths:
  - core parses and semantically projects a `1,200`-level nested Block composite hierarchy;
  - core builds the typed render model for the same hierarchy;
  - render parses, layouts, and renders SVG for the same hierarchy.

## Verification

- `cargo fmt --check -p merman-core -p merman-render` - passed.
- `cargo nextest run -p merman-core block` - passed, `35` tests run.
- `cargo nextest run -p merman-render --test block_svg_test` - passed, `7` tests run.
- `cargo run -p xtask -- compare-block-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed.
- `git diff --check` - passed.

## Notes

- No SVG baseline, root override, Architecture root-bounds formula, or Mermaid parity fixture was
  changed.
- This slice hardens Block's accepted deep composite path. It does not introduce a new Block depth
  limit.
