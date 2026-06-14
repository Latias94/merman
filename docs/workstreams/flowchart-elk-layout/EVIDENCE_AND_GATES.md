# Flowchart ELK Layout - Evidence And Gates

Status: Active
Last updated: 2026-06-14

## Current Evidence

- `crates/merman-bindings-core/src/render.rs` proves `render_svg` returns SVG for
  `flowchart-elk`.
- `crates/merman/tests/flowchart_elk_render.rs` proves the headless renderer can render the smoke
  case with the `elk-layout` feature enabled.
- `crates/merman-layout-elk/src/lib.rs` unit tests prove recursive group layout, nested local
  directions, cross-group routing direction, sibling group separation, and parallel-edge spacing in
  the lightweight backend.
- `crates/merman-render/src/flowchart/elk.rs` unit tests prove Flowchart subgraph directions and
  Mermaid `elk` config fields reach the ELK graph adapter.
- `crates/xtask/src/cmd/compare/xml.rs` keeps the upstream `flowchart-elk` demo fixture out of the
  Flowchart parity matrix on purpose.
- `repo-ref/mermaid/cypress/integration/rendering/flowchart/flowchart-elk.spec.js` provides the
  upstream fixture set to classify.

## Current Gates

```bash
cargo nextest run -p merman-layout-elk
cargo nextest run -p merman-render --features elk-layout flowchart_elk
cargo nextest run -p merman-bindings-core render_svg_returns_svg_for_flowchart_elk
cargo nextest run -p merman flowchart_elk_render
cargo test -p xtask svg_xml_compare_skip_reason
cargo fmt --check
git diff --check
```

## Future Admission Gates

- Tier A smoke fixtures should be admitted with targeted compare coverage before any deeper ELK
  work starts.
- Tier B fixtures should only be admitted after the adapter can explain nested subgraph
  direction, cluster edges, and the visible ordering behavior without ad hoc hacks.
- Tier C fixtures should only move when there is a clear rationale for a deeper ELK port.
