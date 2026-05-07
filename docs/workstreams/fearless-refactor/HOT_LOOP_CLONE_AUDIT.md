# Hot-Loop Clone Audit

Date: 2026-05-08

This note tracks non-JSON clone cleanup in layout/render hot paths. The target is to remove
avoidable allocation without making Mermaid parity logic harder to read.

## Flowchart Self-Loop Expansion

Flowchart layout expands self-loop edges into Mermaid-compatible helper nodes and three
`*-cyclic-special-*` helper edges. Before this audit, the layout path cloned every original
`FlowEdge` into the intermediate `render_edges` vector even when the edge was not a self-loop.

Decision:

- Keep owned helper edges for self-loops because their ids, endpoints, labels, and edge type differ
  from the source edge.
- Store non-self-loop edges as `Cow::Borrowed` in the intermediate layout vector.
- Keep SVG rendering's existing `Cow` approach; it was already borrowing normal edges.
- Construct generated helper edges explicitly instead of cloning the whole source edge and mutating
  ids/endpoints/labels afterward. The helper keeps the intentional layout-vs-SVG difference:
  layout endpoint segments use empty labels, SVG endpoint segments use no labels, and the third
  helper edge preserves the original edge type for marker parity.

Result:

- Normal flowchart layout no longer clones every edge solely to build `render_edges`.
- Self-loop behavior remains explicit and parity-preserving.
- Layout and SVG rendering now share one helper-edge constructor without flattening their
  Mermaid-specific output differences.

## Sequence Block Collection

Sequence block collection (`alt`, `loop`, `critical`, etc.) is consumed immediately during SVG
rendering while the typed render model is still borrowed. It does not need to own block labels or
message ids.

Decision:

- Keep labels and message ids borrowed from `SequenceDiagramRenderModel`.
- Keep owned vectors for block/section structure because the stack needs to move completed block
  scopes into the render list.
- Keep DOM-order-sensitive block rendering unchanged; only the storage ownership changed.

Result:

- `collect_sequence_blocks` no longer copies each block label and message id into `String`.
- Block geometry helpers now consume `&str` iterators directly.

## Remaining Candidates

1. Class namespace/relation lookup construction still clones ids heavily because graphlib-style
   graph APIs own node and edge keys. Only optimize after API boundaries are clearer.

## Verification

- `cargo fmt --check`
- `cargo check -p merman-render --all-targets --all-features`
- `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`
- `cargo nextest run -p merman-render flowchart`
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo nextest run -p merman-render sequence`
- `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity --dom-decimals 3`
