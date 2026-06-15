# Flowchart ELK Layout - Evidence And Gates

Status: Active
Last updated: 2026-06-15

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
- `crates/xtask/src/cmd/upstream_svg_policy.rs` keeps Flowchart ELK admission centralized.
  Current upstream ELK fixtures are probe candidates only, not default parity admissions.
- `crates/xtask/src/cmd/compare/diagrams/flowchart.rs` supports
  `--include-elk-probes` for explicit ELK probe runs without turning the default Flowchart parity
  matrix red.
- `crates/xtask/src/cmd/compare/xml.rs` skips unadmitted Flowchart ELK after parsing the fixture,
  so `flowchart-elk`, `layout: elk`, and `flowchart.defaultRenderer=elk` share the same policy.
- `crates/merman-render/src/svg/parity.rs` preserves `flowchart-elk` as the root
  `aria-roledescription` and marker prefix when rendering a layouted Flowchart ELK diagram.
- Flowchart ELK SVG emission uses Mermaid's root-level group order: marker group, shadow `defs`,
  `subgraphs`, `nodes`, `edges edgePaths`, then `edgeLabels`.
- `crates/merman-elk-layered/src/p1cycles.rs` ports Eclipse ELK's greedy model-order cycle breaker
  tie-break over the existing greedy cycle breaker and proves the lowest model-order candidate is
  chosen before falling back to random selection.
- `crates/merman-elk-layered/src/p3order/sweep.rs` ports Eclipse ELK's barycenter
  `distributePortsWhileSweeping(...)` hook for source-backed P3 sweeps and proves both free-layer
  and fixed-layer ports are redistributed during a sweep.
- `crates/merman-elk-layered/src/p5edges/orthogonal.rs` follows Eclipse ELK's
  `LNode#getPorts(PortType.OUTPUT, side)` semantics for source-backed P5 routing: output ports are
  selected by actual outgoing edge incidence, not by the imported static port marker.
- `https://github.com/mermaid-js/mermaid/blob/develop/cypress/integration/rendering/flowchart/flowchart-elk.spec.js`
  provides the upstream fixture set to classify.

## Current Gates

```bash
cargo nextest run -p merman-layout-elk
cargo nextest run -p merman-elk-layered
cargo nextest run -p merman-render --features elk-layout flowchart_elk
cargo nextest run -p merman-render --features elk-layout render_layouted_svg_preserves_flowchart_elk_roledescription
cargo nextest run -p merman-bindings-core render_svg_returns_svg_for_flowchart_elk
cargo nextest run -p merman --features elk-layout --test flowchart_elk_render
cargo run -p xtask -- compare-flowchart-svgs --filter upstream_html_demos_flowchart_elk_flowchart_elk_001 --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/flowchart_elk_demo_default.md
cargo run -p xtask -- compare-flowchart-svgs --filter upstream_html_demos_flowchart_elk_flowchart_elk_001 --include-elk-probes --flowchart-elk-backend source-ported --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/flowchart_elk_demo_probe_sourceported.md
cargo run -p xtask -- compare-svg-xml --diagram flowchart --filter upstream_html_demos_flowchart_elk_flowchart_elk_001 --check --dom-mode parity --dom-decimals 3
cargo test -p xtask svg_xml_compare_skip_reason
cargo fmt --check
git diff --check
```

## Probe Lane

Explicit probe command:

```bash
cargo run -p xtask -- compare-flowchart-svgs --filter upstream_html_demos_flowchart_elk_flowchart_elk_001 --include-elk-probes --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/flowchart_elk_demo_probe.md
```

Current result:

- Default compat compare still skips the fixture with the centralized local-policy reason and
  returns success.
- Explicit source-backed probe now returns `All fixtures matched` for the HTML ELK demo fixture.
- The last resolved geometry gap was P5 route-slot construction: Eclipse ELK filters
  `PortType.OUTPUT` ports by actual outgoing edges, while the Rust port had been checking the
  static imported port marker.

## Future Admission Gates

- Tier A smoke fixtures should be admitted with targeted compare coverage before any deeper ELK
  work starts.
- Tier B fixtures should only be admitted after the adapter can explain nested subgraph
  direction, cluster edges, and the visible ordering behavior without ad hoc hacks.
- Tier C fixtures should only move when there is a clear rationale for a deeper ELK port.
