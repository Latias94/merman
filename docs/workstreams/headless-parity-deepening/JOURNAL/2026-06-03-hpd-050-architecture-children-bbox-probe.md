# HPD-050 - Architecture Children BBox Probe Audit

Date: 2026-06-03

## Context

The previous group bbox source-formula audit split the two active `+5px` Architecture rows into a
child-contribution drift plus a final group formula drift:

- `stress_architecture_batch5_long_titles_and_punct_076`
- `stress_architecture_html_titles_and_escapes_041`

That audit still inferred the exact child contribution from parent `autoWidth` and final
`node.boundingBox()` output. This pass extends the browser probe so the Cytoscape children bbox
used by `updateCompoundBounds()` is recorded directly.

## Probe Change

`tools/debug/arch_fcose_browser_probe_fixture_025.js` now records two parent-only metrics in each
node dump:

- `childrenBoundingBoxIncludeLabels`: `n.children().boundingBox({ includeLabels: true, includeOverlays: false, useCache: false })`
- `childrenBoundingBoxBodyOnly`: `n.children().boundingBox({ includeLabels: false, includeOverlays: false, useCache: false })`

The fields are intentionally emitted only by the diagnostic probe. They are not a renderer
contract and do not make the manual probe an authoritative Mermaid CLI render.

## Source Findings

The pinned Cytoscape source confirms the phase split:

- `updateCompoundBounds()` computes parent `_p.autoWidth` / `_p.autoHeight` from
  `children.boundingBox({ includeLabels, includeOverlays: false, useCache: false })`.
- Parent `width()` and `height()` read those auto dimensions.
- Parent `outerWidth()` and `outerHeight()` add border plus `2 * padding()`.
- Node label bounds use the rendered label width/height and then apply a `marginOfError = 2` on
  both sides, so a plain centered service label's `labelBounds.w` is `labelWidth + 4`.
- The default `node.boundingBox()` body path is later and includes another visual expansion phase.

This means the production approximation needs a child-label-bounds phase before any final group
body expansion is applied.

## Evidence

Browser probe output for `batch5` group `pipeline`:

- `childrenBoundingBoxIncludeLabels.w=379.926`, exactly matching `autoWidth=379.926`.
- `childrenBoundingBoxBodyOnly.w=282.926`.
- `outerWidth=460.926`, final `node.boundingBox().w=462.926`.
- Service `labelWidth` / `labelBounds.w`:
  - `Runner Linux amd64`: `149` / `153`
  - `Container Registry`: `133` / `137`
  - `Artifacts Storage retention 30d`: `217` / `221`
  - `Production`: `77` / `81`

Browser probe output for `html_titles` group `ui`:

- `childrenBoundingBoxIncludeLabels.w=316.926`, exactly matching `autoWidth=316.926`.
- `childrenBoundingBoxBodyOnly.w=282.926`.
- `outerWidth=397.926`, final `node.boundingBox().w=399.926`.
- Service `labelWidth` / `labelBounds.w`:
  - `Web Front Line 2`: `123` / `127`
  - `CDN Cache`: `86` / `90`
  - `Origin primary`: `101` / `105`

Rust-side `[arch-cy-bbox]` debug for the same labels shows that the current
`architecture_cytoscape_canvas_label_metrics(...)` approximation is close but not a single
offset from browser label bounds:

- `Runner Linux amd64`: local child-label contribution `154` vs browser `153`
- `Container Registry`: local `139` vs browser `137`
- `Artifacts Storage retention 30d`: local `225` vs browser `221`
- `Production`: local `81` vs browser `81`
- `Web Front Line 2`: local `129` vs browser `127`
- `CDN Cache`: local `82` vs browser `90`
- `Origin primary`: local `109` vs browser `105`

The aggregate parent drift on the two focused rows is still real, but these per-label facts reject
a uniform subtract-N or group-padding correction. Some labels are wider locally, one is narrower
locally, and only the union-controlling labels decide the final parent `autoWidth`.

## Outcome

No production behavior changed. The new probe makes the next implementation target more precise:

1. model Cytoscape service child contribution as a source-backed union of body bounds and
   labelBounds, not default `node.boundingBox()`;
2. keep SVG root `createText(...)` measurement separate from Cytoscape compound child measurement;
3. validate any helper against the full Architecture root suite before accepting it, because the
   same text-measurement seam also feeds FCoSE node bounds.

Until a phase-specific helper is broad enough to survive the full suite, keep the current
`ARCHITECTURE_SVG_GROUP_BBOX_EXTRA_PADDING_PX=2.5` approximation and classify these rows as
Architecture Cytoscape children-bbox / final-group-bbox phase residuals.

## Verification

- `$env:PUPPETEER_EXECUTABLE_PATH='C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'; node tools/debug/arch_fcose_browser_probe_fixture_025.js stress_architecture_batch5_long_titles_and_punct_076 > target/compare/arch_batch5_long_titles_probe_hpd050_children_bbox.json`
- `$env:PUPPETEER_EXECUTABLE_PATH='C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'; node tools/debug/arch_fcose_browser_probe_fixture_025.js stress_architecture_html_titles_and_escapes_041 > target/compare/arch_html_titles_probe_hpd050_children_bbox.json`
- `$env:MERMAN_ARCH_DEBUG_CY_BBOX='1'; cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch5_long_titles_and_punct_076 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch5_hpd050_children_bbox_debug.md`
- `$env:MERMAN_ARCH_DEBUG_CY_BBOX='1'; cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_html_titles_and_escapes_041 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_html_titles_hpd050_children_bbox_debug.md`
