# HPD-090 Flowchart HTML KaTeX Baseline Refresh

Flowchart HTML demo KaTeX fixtures were the last narrow stale stored-SVG set in HPD-090.

Outcome:

- Point-refreshed four stale Flowchart upstream SVG baselines:
  `upstream_html_demos_flowchart_flowchart_040_katex`,
  `upstream_html_demos_flowchart_flowchart_042_katex`,
  `upstream_html_demos_flowchart_flowchart_044_katex`, and
  `upstream_html_demos_flowchart_graph_039_katex`.
- The refresh is baseline-only. Generated Mermaid 11.15 output now carries the current `_katex`
  diagram id suffix, marker margin defs, `data-look="classic"` DOM, `1px` shared edge width,
  neo/drop-shadow CSS rules, and current KaTeX MathML measurement output.
- Local Flowchart output already matched the refreshed baselines under DOM parity; no renderer code
  change was needed.
- `update-layout-snapshots --diagram flowchart` added the missing existing-fixture
  `zed_pr_57644_flowchart.layout.golden.json` snapshot.

Verification:

- `$env:PUPPETEER_EXECUTABLE_PATH='C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'; cargo run -p xtask -- check-upstream-svgs --diagram flowchart --filter upstream_html_demos_flowchart --check-dom --dom-mode structure --dom-decimals 3` -
  passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_html_demos_flowchart --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\flowchart_report_parity_hpd090_html_katex.md` -
  passed; the known ELK fixture remains skipped as unsupported layout.
- `cargo run -p xtask -- update-layout-snapshots --diagram flowchart` - passed and produced the
  Flowchart layout golden addition above.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\flowchart_report_parity_hpd090_html_katex_full.md` -
  passed for the full Flowchart family.
- `cargo nextest run -p merman-render --test layout_snapshots_test fixtures_match_layout_golden_snapshots_when_present` -
  passed, `1` test run.

Residual note:

- HPD-090 has no known broad or narrow stale stored-SVG set remaining. Next work is readiness gate
  revalidation and documenting expected diagnostics before returning to parity/root residual fixes.
