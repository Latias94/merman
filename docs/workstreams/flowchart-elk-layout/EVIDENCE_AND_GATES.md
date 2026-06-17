# Flowchart ELK Layout - Evidence And Gates

Status: Active
Last updated: 2026-06-17

## Current Evidence

- `crates/merman-bindings-core/src/render.rs` proves `render_svg` returns SVG for
  `flowchart-elk`.
- `crates/merman/tests/flowchart_elk_render.rs` proves the headless renderer can render the smoke
  case with the `elk-layout` feature enabled.
- `crates/merman-layout-elk/src/lib.rs` unit tests prove recursive group layout, nested local
  directions, cross-group routing direction, sibling group separation, and parallel-edge spacing in
  the compatibility backend.
- `crates/merman-render/src/flowchart/elk.rs` unit tests prove Flowchart subgraph directions and
  Mermaid `elk` config fields reach the ELK graph adapter.
- `crates/xtask/src/cmd/upstream_svg_policy.rs` keeps Flowchart ELK admission centralized.
  Current upstream ELK fixtures are source-backed probe admissions only, not default parity
  admissions.
- `crates/xtask/src/cmd/compare/diagrams/flowchart.rs` defaults Flowchart ELK diagnostics to the
  source-backed backend, supports explicit `--flowchart-elk-backend compat` fallback, and exposes
  `check-flowchart-elk-source-backed-probes` as the fixed source-backed probe gate.
- `crates/xtask/src/cmd/compare/xml.rs` skips unadmitted Flowchart ELK after parsing the fixture,
  so `flowchart-elk`, `layout: elk`, and `flowchart.defaultRenderer=elk` share the same backend-
  aware policy.
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
cargo nextest run -p xtask source_backed_elk_probe_matches_html_demo_fixture
cargo run -p xtask -- compare-flowchart-svgs --filter upstream_html_demos_flowchart_elk_flowchart_elk_001 --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/flowchart_elk_demo_default.md
cargo run -p xtask -- check-flowchart-elk-source-backed-probes
cargo run -p xtask -- compare-svg-xml --diagram flowchart --filter upstream_html_demos_flowchart_elk_flowchart_elk_001 --check --dom-mode parity --dom-decimals 3
cargo test -p xtask svg_xml_compare_skip_reason
cargo fmt --check
git diff --check
```

## Probe Lane

Explicit probe command:

```bash
cargo run -p xtask -- check-flowchart-elk-source-backed-probes
```

Current result:

- Default compare/debug/XML tools now use the source-backed backend for Flowchart ELK; `compat`
  remains an explicit fallback.
- Registered Flowchart ELK probe fixtures are admitted only when `--include-elk-probes` is set and
  the active backend is source-backed, so backend defaulting does not broaden SVG parity admission
  by itself.
- The source-backed probe gate returns `All fixtures matched` for the admitted probe list.
- The last resolved geometry gap was P5 route-slot construction: Eclipse ELK filters
  `PortType.OUTPUT` ports by actual outgoing edges, while the Rust port had been checking the
  static imported port marker.

## Future Admission Gates

- Decide when the 63 admitted source-backed probe fixtures should enter the broad Flowchart matrix.
- Keep duplicate-body exact-call fixtures in the source-backed lane for upstream traceability while
  using the 57 unique layout body count for semantic coverage.
- Keep future ELK user/upstream regressions source-backed; do not tune geometry from fixture output.
