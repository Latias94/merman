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

## Remaining Candidates

1. Sequence block/message label assembly still has several `String` clones around block collection
   and label wrapping. Audit only after isolating DOM-order-sensitive paths.
2. Class namespace/relation lookup construction still clones ids heavily because graphlib-style
   graph APIs own node and edge keys. Only optimize after API boundaries are clearer.

## Verification

- `cargo fmt --check`
- `cargo check -p merman-render --all-targets --all-features`
- `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`
- `cargo nextest run -p merman-render flowchart`
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3`
