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
- `crates/xtask/src/cmd/upstream_svg_policy.rs` keeps Flowchart ELK admission centralized.
  Current upstream ELK fixtures are probe candidates only, not default parity admissions.
- `crates/xtask/src/cmd/compare/diagrams/flowchart.rs` supports
  `--include-elk-probes` for explicit ELK probe runs without turning the default Flowchart parity
  matrix red.
- `crates/xtask/src/cmd/compare/xml.rs` skips unadmitted Flowchart ELK after parsing the fixture,
  so `flowchart-elk`, `layout: elk`, and `flowchart.defaultRenderer=elk` share the same policy.
- `crates/merman-render/src/svg/parity.rs` preserves `flowchart-elk` as the root
  `aria-roledescription` and marker prefix when rendering a layouted Flowchart ELK diagram.
- `repo-ref/mermaid/cypress/integration/rendering/flowchart/flowchart-elk.spec.js` provides the
  upstream fixture set to classify.

## Current Gates

```bash
cargo nextest run -p merman-layout-elk
cargo nextest run -p merman-render --features elk-layout flowchart_elk
cargo nextest run -p merman-render --features elk-layout render_layouted_svg_preserves_flowchart_elk_roledescription
cargo nextest run -p merman-bindings-core render_svg_returns_svg_for_flowchart_elk
cargo nextest run -p merman flowchart_elk_render
cargo run -p xtask -- compare-flowchart-svgs --filter upstream_html_demos_flowchart_elk_flowchart_elk_001 --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/flowchart_elk_demo_default.md
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

- Default compare skips the fixture with the centralized local-policy reason and returns success.
- Explicit probe fails at DOM parity. The first mismatch is still root-shape related:
  upstream emits root-level `marker` elements before shadow `defs`, then `subgraphs`, `nodes`,
  `edges edgePaths`, and `edgeLabels`; local output still uses the Flowchart V2 wrapper
  `<g><g class="root">...`.
- The remaining geometry is a real layout gap, not only DOM wrapping. Upstream places `C`,
  `D/I/E`, `F/H/G`, and the feedback edge across multiple columns with orthogonal routing; the
  lightweight backend still stacks most nodes vertically for this probe.

## Future Admission Gates

- Tier A smoke fixtures should be admitted with targeted compare coverage before any deeper ELK
  work starts.
- Tier B fixtures should only be admitted after the adapter can explain nested subgraph
  direction, cluster edges, and the visible ordering behavior without ad hoc hacks.
- Tier C fixtures should only move when there is a clear rationale for a deeper ELK port.
