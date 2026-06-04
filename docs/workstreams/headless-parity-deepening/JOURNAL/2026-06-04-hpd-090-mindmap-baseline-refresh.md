# HPD-090 Mindmap Baseline Refresh

Mindmap was part of the broad stale stored-SVG set for Mermaid 11.15 baseline preparation.

Outcome:

- Regenerated all `114` `fixtures/upstream-svgs/mindmap/*.svg` files to the pinned Mermaid 11.15
  baseline.
- Added the missing `fixtures/mindmap/zed_pr_57644_mindmap.layout.golden.json` layout golden.
- DOM parity was not a pure fixture refresh. Mermaid 11.15 Mindmap output now requires scoped
  shadow defs, explicit classic `data-look`, diagram-prefixed node/edge DOM ids, margin markers,
  section class cycle wrapping, and direct classic rounded/hexagon shape DOM.
- Updated `crates/merman-render/src/svg/parity/mindmap.rs` so:
  - node group ids, default rounded path ids, and edge path ids use `<diagram-id>-<raw-id>`;
  - edge `data-id` keeps the raw edge id for semantic tooling;
  - node and edge `section-*` classes wrap after Mermaid's `0..10` color cycle;
  - classic rounded nodes render direct `<rect>` DOM and classic hexagons render direct
    `<polygon>` DOM instead of the stale rough-wrapper structure;
  - XHTML labels keep the current `nodeLabel markdown-node-label` class tokens.
- Removed the now-unused local `roughjs46` compatibility helper.

Verification:

- `cargo nextest run -p merman-render --test mindmap_svg_test` - passed, `3` tests run.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke mindmap` -
  passed, `2` tests run.
- `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed.
- `cargo nextest run -p merman-render --test layout_snapshots_test fixtures_match_layout_golden_snapshots_when_present` -
  passed, `1` test run.
- `cargo fmt --check -p merman-render -p merman` - passed.

Residual note:

- Mindmap is removed from the HPD-090 broad stale queue. The remaining broad stale family is
  `radar`, followed by narrow refreshes for `class`, `timeline`, and Flowchart HTML demo KaTeX
  fixtures.
