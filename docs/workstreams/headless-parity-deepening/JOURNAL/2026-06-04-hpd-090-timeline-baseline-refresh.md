# HPD-090 Timeline Baseline Refresh

Timeline was the second narrow stale stored-SVG set after the broad stale family queue and Class
were cleared.

Outcome:

- Point-refreshed the one stale Timeline upstream SVG baseline:
  `upstream_cypress_timeline_spec_12_should_render_timeline_with_proper_vertical_line_lengths_for_012`.
- DOM parity was not a pure fixture refresh. The stale fixture sets `fontFamily: Fira Sans` and
  `fontSize: 17px`; in the pinned Mermaid 11.15 Edge/Puppeteer baseline environment, bare
  `Fira Sans` resolves through the browser sans-serif fallback for SVG text measurement.
- Updated Timeline's local measurement seam so only this Timeline browser-fallback case uses
  sans-serif wrap metrics and the observed `25px` first-line bbox height lattice.
- Added focused coverage for `Quality Management System (4)` so the local layout keeps the
  browser-backed three-line wrap and `91.2px` virtual node height.
- Refreshed the affected Timeline layout golden and added the missing existing-fixture
  `zed_pr_57644_timeline.layout.golden.json` snapshot.

Verification:

- `cargo nextest run -p merman-render fira_sans_17_timeline_metrics_match_mermaid_browser_wrap` -
  passed, `1` test run.
- `cargo nextest run -p merman-render --test timeline_svg_test` - passed.
- `cargo run -p xtask -- update-layout-snapshots --diagram timeline` - passed and produced the
  Timeline layout golden updates above.
- `cargo nextest run -p merman-render --test layout_snapshots_test fixtures_match_layout_golden_snapshots_when_present` -
  passed, `1` test run.
- `cargo run -p xtask -- compare-timeline-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\timeline_report_parity_hpd090_after_fira_sans_measurement.md` -
  passed.
- `cargo fmt -p merman-render --check` - passed.

Residual note:

- Timeline structural DOM parity is green after the narrow refresh. Remaining HPD-090 narrow work
  is Flowchart HTML demo KaTeX drift (`4` fixtures), followed by readiness gates.
