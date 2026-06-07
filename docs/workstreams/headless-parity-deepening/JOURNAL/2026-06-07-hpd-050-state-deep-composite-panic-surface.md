# HPD-050 - State Deep-Composite Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

After C4's deep boundary traversal was converted away from recursive layout calls, the next
panic-surface audit targeted State. Public `stateDiagram-v2` syntax can construct arbitrarily deep
composite state hierarchies. A `1,500`-level composite chain reproduced stack overflow through
`parse_diagram_for_render_model_sync(...)`; an initial render-side cluster-copy conversion was not
enough because parse-only still overflowed.

## Changes

- Replaced State DB extraction's deep `root_doc.clone()` path with borrowed traversal of the parsed
  root document.
- Removed recursively cloned composite `doc` subtrees from `StateRecord`; semantic compatibility
  `doc` output is now generated from the AST only when semantic JSON is requested.
- Built State semantic `states[*].doc` and the final semantic root object with explicit stacks and
  hand-built `serde_json::Map` values, avoiding recursive `json!` wrapping of deep `Value` trees.
- Added explicit-stack cleanup for `StateDb` so dropping a successfully parsed deep AST does not
  recurse through nested `Vec<Stmt>` values.
- Converted State cluster extraction, cluster preparation, nested prepared-graph layout, and
  prepared-graph cleanup away from recursive Rust call-stack traversal.
- Removed the old `prepare_graph(...)` 10-level bailout that could leave a very deep compound State
  graph for Dagre to handle directly.
- Added public-path regressions covering:
  - `1,200`-level core semantic JSON projection;
  - `1,200`-level core typed render-model parsing;
  - `1,500`-level render-model parse-only;
  - `512`-level render layout through `layout_parsed(...)`.

## Verification

- `cargo nextest run -p merman-render --test state_layout_test state_parse_for_render_model_handles_deep_composite_chain` -
  first run failed before the core fix with stack overflow; passed after the non-recursive State DB
  extraction/projection/drop changes.
- `cargo nextest run -p merman-core state_deep_composite_chain_semantic_and_render_model_use_heap_traversal` -
  first run failed before replacing the final semantic `json!` wrapper; passed after hand-built
  semantic root assembly.
- `cargo nextest run -p merman-render --test state_layout_test state_layout_handles_deep_composite_chain` -
  passed after explicit-stack cluster preparation and layout traversal.
- `cargo nextest run -p merman-core state` - passed, `39` tests run.
- `cargo nextest run -p merman-render state` - passed, `17` tests run.
- `cargo fmt --check -p merman-core -p merman-render` - passed.
- `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed.

## Notes

- No SVG baseline, root override, Architecture root-bounds formula, or Mermaid parity fixture was
  changed.
- This slice hardens State's accepted deep composite path. It does not introduce a new State depth
  limit.
