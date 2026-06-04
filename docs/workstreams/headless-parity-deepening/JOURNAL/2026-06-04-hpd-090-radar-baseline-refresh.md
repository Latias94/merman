# HPD-090 Radar Baseline Refresh

Radar was the last broad stale stored-SVG family in the Mermaid 11.15 baseline preparation queue.

Outcome:

- Regenerated all `41` `fixtures/upstream-svgs/radar/*.svg` files to the pinned Mermaid 11.15
  baseline.
- DOM parity was not a pure fixture refresh. Mermaid 11.15 Radar roots now emit responsive
  `width="100%"`, a root `max-width: <width>px` style, and no fixed root `height` attribute.
- Updated `crates/merman-render/src/svg/parity/radar.rs` so local Radar root output uses the same
  current root DOM shape.
- Removed the now-unused `SvgRootFixedHeightPlacement::AfterViewBox` helper branch after Radar
  stopped emitting a fixed root height.
- `cargo run -p xtask -- update-layout-snapshots --diagram radar` produced no committed layout
  snapshot changes.

Verification:

- `cargo fmt -p merman-render --check` - passed.
- `cargo nextest run -p merman-render radar` - passed, `3` tests run.
- `cargo run -p xtask -- update-layout-snapshots --diagram radar` - passed with no committed
  layout snapshot changes.
- `cargo run -p xtask -- compare-radar-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed.

Residual note:

- The broad stale family queue is now empty. Remaining HPD-090 work is narrow stored-SVG refresh for
  `class`, `timeline`, and Flowchart HTML demo KaTeX fixtures, followed by readiness gates.
