# HPD-050 - Architecture Cytoscape Canvas Width Audit

Date: 2026-06-04
Task: HPD-050 Architecture-first layout engine audit

## Context

After the narrow Procrustes compatibility slice removed `group_port_edges_017`, the remaining
Architecture root queue still contains direct group/service width tails such as:

- `stress_architecture_batch5_long_titles_and_punct_076`
- `stress_architecture_html_titles_and_escapes_041`
- `stress_architecture_unicode_and_xml_escapes_019`

The next question was whether those rows were caused by measuring Architecture service titles with
the wrong rendered SVG font/style.

## Findings

- Actual stored upstream and local SVG service titles inherit the root Architecture SVG style:
  `font-family: "trebuchet ms", verdana, arial, sans-serif; font-size: 16px;`.
- Pinned Mermaid Architecture SVG source still emits service/group titles through `createText(...)`
  without a service-label-specific font override.
- The Cytoscape layout stylesheet is a different phase: `node[label]` sets only `font-size`, not
  `font-family`.
- Cytoscape's own default `font-family` is `Helvetica Neue, Helvetica, sans-serif`.
- Cytoscape label dimensions use canvas `Math.ceil(ctx.measureText(...).width)`, and label bounds
  add a `2px` margin-of-error on both left and right, so a centered service label's
  `labelBounds.w` is `labelWidth + 4`.
- An Edge/Puppeteer canvas probe using
  `normal 400 16px Helvetica Neue, Helvetica, sans-serif` exactly matched the browser/Cytoscape
  probe label widths:
  - `Runner Linux amd64`: `149`
  - `Container Registry`: `133`
  - `Artifacts Storage retention 30d`: `217`
  - `Production`: `77`
  - `Web Front Line 2`: `123`
  - `CDN Cache`: `86`
  - `Origin primary`: `101`

This confirms the source rule, but it does not by itself justify a production change.

## Rejected Experiments

1. Cytoscape default font family as the local compound-label `TextStyle`.
   - Temporary patch: changed only the Architecture compound/FCoSE label measurement font to
     `Helvetica Neue, Helvetica, sans-serif`.
   - Result: representative rows worsened:
     - `batch5`: `+5px` to `+9.5px`
     - `html_titles`: `+5px` to `+9px`
     - `unicode`: `+3px` to `+6.5px`
   - Reason: local vendored Arial/Helvetica metrics do not match Edge canvas Arial metrics.

2. Generated exact Cytoscape `labelWidth` table plus source `labelBounds` half-width
   (`labelWidth / 2 + 2`), keeping existing final group padding.
   - Temporary table covered `169` unique Architecture service titles from fixtures.
   - Result: representative rows improved but did not close:
     - `batch5`: `+5px` to `+2px`
     - `html_titles`: `+5px` to `+2px`
     - `unicode`: `+3px` to `+2px`
   - Full Architecture root still failed with `25` mismatches, worse than the current `24` queue,
     and `batch6_init_fontsize_icon_size_wrap_093` shifted to `-8px`.
   - Reason: exact child label width alone is another half-source model; body and final group phases
     still use the older compensation model.

3. Exact Cytoscape `labelWidth` table plus final group extra padding `2.5px -> 1.5px`.
   - Result: representative row widths became exact, but heights immediately became `2px` short.
   - Reason: this repeats the previously rejected split-axis/group-padding direction. It moves the
     residual from width to height instead of modeling the final compound phase.

All temporary production patches were reverted. The code worktree returned to clean state before
this journal entry was written.

## Outcome

No production behavior changed.

The service title SVG style suspicion is ruled out. The source-backed measurement fact is now
clear: Architecture has two separate text phases:

1. SVG `createText(...)` labels inherit Mermaid's SVG root font.
2. Cytoscape compound child labels use Cytoscape's canvas/default-font label metrics.

The next safe production path is not a font-family switch, a global font table rebuild, or a
labelWidth lookup alone. A candidate fix must model the child body, child label, final group
`node.boundingBox()`, and root SVG consumption phases together and survive full Architecture root
verification.

## Verification

- Edge/Puppeteer canvas measurement for the seven focused service labels - passed; widths matched
  the existing browser/Cytoscape probe.
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch5_long_titles_and_punct_076 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_batch5_hpd050_cytoscape_font_experiment.md` -
  expected failure; font-family switch worsened width delta to `+9.5px`.
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_html_titles_and_escapes_041 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_html_hpd050_cytoscape_font_experiment.md` -
  expected failure; font-family switch worsened width delta to `+9px`.
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_unicode_and_xml_escapes_019 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_unicode_hpd050_cytoscape_font_experiment.md` -
  expected failure; font-family switch worsened width delta to `+6.5px`.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_hpd050_cytoscape_lookup_experiment.md` -
  expected failure; exact labelWidth lookup plus old final group phase produced `25` mismatches.
- Focused exact labelWidth plus `1.5px` group-extra experiments - expected failures; widths became
  exact but heights became `2px` short on the representative rows.
