# HPD-050 - Architecture Group BBox Source Formula Audit

Date: 2026-06-03

## Context

After classifying the junction row, the next active Architecture residuals were the two `+5px`
group/service bbox rows:

- `stress_architecture_batch5_long_titles_and_punct_076`
- `stress_architecture_html_titles_and_escapes_041`

Earlier evidence proved that globally removing the final SVG group bbox extra makes these rows
width-exact but breaks height and regresses many group-heavy fixtures. This pass compared current
local group content bounds against browser `finalElements` metrics and Cytoscape source to decide
whether a source-backed narrower fix exists.

## Source Findings

Pinned Mermaid 11.15 Architecture renders group rectangles from Cytoscape final
`node.boundingBox()`. The relevant Cytoscape source is in
`tools/mermaid-cli/node_modules/cytoscape/src/collection/dimensions/bounds.mjs` and
`width-height.mjs`:

- `updateCompoundBounds()` sizes a parent from `children.boundingBox({ includeLabels, includeOverlays: false, useCache: false })`.
- With `compound-sizing-wrt-labels: include`, child labels participate in that children bbox.
- Parent `_p.autoPadding` is computed from the CSS `padding` property; for Mermaid Architecture
  this is a pixel value, so it stays at the configured padding.
- Parent `width()` / `height()` return `_p.autoWidth` / `_p.autoHeight`.
- Parent `outerWidth()` / `outerHeight()` then add border plus `2 * padding()`.
- The default `node.boundingBox()` body path stores body bounds and applies the final visual
  expansion used for browser inaccuracies / antialiasing.

This means the local approximation needs to distinguish child contribution into
`children.boundingBox(...)` from the later final group `node.boundingBox()` formula. A single
scalar `content +/- pad` cannot represent both phases exactly.

## Evidence

Current local reports after reverting experiments remain expected failures:

- `target/compare/architecture_batch5_hpd050_followup_current_after_revert.md`: upstream
  `542.926x462.926`, local `547.926x462.926`.
- `target/compare/architecture_html_titles_hpd050_followup_current_after_revert.md`: upstream
  `479.926x462.926`, local `484.926x462.926`.

Browser `finalElements` metrics:

- `batch5` group `pipeline`: `autoWidth=379.926`, `outerWidth=460.926`,
  `node.boundingBox().w=462.926`.
- `html_titles` group `ui`: `autoWidth=316.926`, `outerWidth=397.926`,
  `node.boundingBox().w=399.926`.

Local group debug:

- `batch5` group `pipeline`: content width `382.926`, final width `467.926`.
- `html_titles` group `ui`: content width `319.926`, final width `404.926`.

The `+5px` rows therefore decompose into two visible pieces:

- local child contribution into the group content width is `+3px` wider than the browser
  `autoWidth`;
- the current final group approximation adds `85px` (`2 * (padding + 2.5)`) rather than the
  browser final contribution of `83px` (`outerWidth - autoWidth + final body expansion`).

## Rejected Experiments

Split-axis group padding experiment:

- Temporary patch used horizontal `padding` and vertical `padding + 2.5`.
- Both focused `+5px` rows became root-green.
- Full Architecture `parity-root` regressed many group-heavy fixtures into too-narrow local
  max-widths, so this is not a valid global model.

Standalone final group extra `+1.5` experiment:

- Temporary patch changed `ARCHITECTURE_SVG_GROUP_BBOX_EXTRA_PADDING_PX` from `2.5` to `1.5`.
- The two focused rows improved from `+5px` to `+3px`, matching the source formula split.
- Full Architecture `parity-root` still reopened many previously green or classified rows, so this
  cannot be accepted without fixing the child contribution phase first.

Both production patches were reverted before commit.

## Outcome

No production behavior changed. The two rows remain classified as Architecture Cytoscape
children-bbox / final-group-bbox phase residuals. The source-backed next implementation target is
not another group padding constant; it is a proper model of Cytoscape
`children.boundingBox({ includeLabels: true, includeOverlays: false })` for service child
contribution, then applying the final group `outerWidth + body expansion` formula.

Until that phase split exists, keep `ARCHITECTURE_SVG_GROUP_BBOX_EXTRA_PADDING_PX=2.5` because it
is the least-regressive production approximation under the current Architecture root suite.

## Verification

- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch5_long_titles_and_punct_076 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch5_hpd050_followup_current.md`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_html_titles_and_escapes_041 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_html_titles_hpd050_followup_current.md`
- `$env:PUPPETEER_EXECUTABLE_PATH='C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'; node tools/debug/arch_fcose_browser_probe_fixture_025.js stress_architecture_html_titles_and_escapes_041 > target/compare/arch_html_titles_probe_hpd050_final_elements.json`
- `$env:MERMAN_ARCH_DEBUG_GROUP_RECT='pipeline'; cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch5_long_titles_and_punct_076 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch5_hpd050_group_rect_debug.md`
- `$env:MERMAN_ARCH_DEBUG_GROUP_RECT='ui'; cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_html_titles_and_escapes_041 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_html_titles_hpd050_group_rect_debug.md`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch5_long_titles_and_punct_076 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch5_hpd050_split_axis_group_pad_experiment.md`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_html_titles_and_escapes_041 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_html_titles_hpd050_split_axis_group_pad_experiment.md`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_hpd050_split_axis_group_pad_experiment.md`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch5_long_titles_and_punct_076 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch5_hpd050_group_extra_1_5_experiment.md`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_html_titles_and_escapes_041 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_html_titles_hpd050_group_extra_1_5_experiment.md`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_hpd050_group_extra_1_5_experiment.md`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch5_long_titles_and_punct_076 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch5_hpd050_followup_current_after_revert.md`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_html_titles_and_escapes_041 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_html_titles_hpd050_followup_current_after_revert.md`
