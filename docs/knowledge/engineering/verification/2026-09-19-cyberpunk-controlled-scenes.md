# Cyberpunk complete scenes with controlled caller fonts

Date: 2026-09-19. Implementation baseline: `a40c505df`.
Scope: U1/U6/U7/U8 scene evidence under the
[theme product boundary plan](../../../plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md).
This record does not close those units or C7a and does not promote catalog cells.

## Capture contract

All three inputs are the unchanged `public-cyberpunk` fixtures in
`crates/merman-theme-fixtures`. Flowchart is captured in HTML and native SVG text modes.
The reference recipe is loaded directly from Modern Mermaid commit
`a021cbce37fc0b07a9f4791c28e983101ea06f2d`; `src/utils/themes.ts` has SHA-256
`9f1baf22457de1fbc5217437dc103840a14f15f2213eeaba45f91e306f6db92d`.
The reference engine is **Mermaid 11.17.2**, matching the current parity baseline, rather than
11.12.1 from the earlier Sequence observation. Chromium is **151.0.7922.34**.

The experiment supplies the same local Arial regular/bold bytes to both browsers and the
native embedded-only profile. Font hashes and sizes match the
[earlier XY capture](2026-09-18-xychart-axis-label-glow.md#controlled-font-capture-contract).
CDP confirms custom ArialMT/Arial-BoldMT on actual text leaves, including nested tspans.
No font is added to the repository or production package.

The public recipe is exported through `export_preset(Cyberpunk)`. The only controlled-profile
changes are its default font stack and caller-owned font assets. Its compiled fingerprint is
`e657c54e847f9f283e8d70b383cd0466b732c907a0fb53fb1aac2e1a814c26fa`.
The resource-free public recipe is retained separately. This is an author-supplied font variant;
it does not certify the unmodified builtin recipe as Portable on arbitrary hosts.

Browser captures use 1280×960 CSS pixels, DPR 1, loaded fonts, blocked network requests and one
SVG unit per CSS pixel. Each surface has 80px exterior inspection padding. The reference's
preview background is composed outside its SVG; Merman's background is part of its root SVG.
Neither the background extent nor layout differences are normalized away. This is a direct
recipe/engine capture, not the complete reference React application.

Native artifacts come from a fresh serial Release build of `merman` with no defaults and
`svg,png,pdf,layout-cytoscape,embedded-fonts`. Export options and resource limits remain default.
PDFs were actually rasterized at 96 dpi with PDFium library SHA-256
`6dbc4ceaa40178e3b583a51144cccd7900a19608fd71f45ecca4a6c766d8024b`.
PDF points and SVG pixels have different output dimensions; the raster is not a same-pixel
comparison to PNG.

## Native outcomes

| Scene/profile | PNG and PDF admission | Filtered groups / primitives |
| --- | --- | --- |
| Flowchart native SVG text, supplied fonts | Portable, Embedded, no reasons | 13 / 68 |
| Sequence, supplied fonts | Portable, Embedded, no reasons | 21 / 116 |
| XY Chart, supplied fonts | Portable, Embedded, no reasons | 28 / 112 |
| Flowchart HTML, host measurement/fonts | HostDependent; SystemOrHostFontDependency only | 13 / 68 |

The first embedded-only Flowchart HTML request returned
`Svg(TextLayout(UnsupportedLabelMode))` before artifact creation. That failed capture remains in
`native-controlled.log`. A separate request without embedded assets, retaining Arial typography,
produced the last row. Its browser capture uses the controlled font bytes, but its native font
source remains System. It must not inherit the other rows' portable-font guarantee. Supporting
HTML with the embedded-only preparation path is not established by this experiment.

The Document request projects HTML labels into ResvgSafe native text for export. Its
`flowchart-html-merman.svg` therefore contains no `foreignObject`; it is not raw HTML evidence.
A separate `RenderRequest::svg` with the parity pipeline produces
`flowchart-html-merman-raw.svg`, retaining HTML labels. The final browser comparison uses this
raw SVG with the supplied browser font bytes. The earlier projected browser capture is retained
as `flowchart-html-merman-document-chromium.png` and `document-browser-observations.json`.
This distinction was raised during independent review and corrected before this record.

## Observed terminals and deliberate differences

- Flowchart has all four navy/cyan node shapes with 3px strokes and ordered shape glow; all
  three rectangles have 10px radii while the diamond retains polygon geometry. Three directed
  edges have 2px cyan strokes and glow. Six node/edge labels have 600 weight and text-only glow.
  Both Yes/No backgrounds remain navy, the Checkout title is readable, and all three actual
  marker references resolve to cyan arrowheads. Unused default marker definitions are not
  confused with those bound markers.
- The reference's `.edgePath .path` and `.arrowheadPath` match zero elements in both Flowchart
  modes. Its edges have 1px strokes without glow, despite the intended recipe. Its native
  text leaves have explicit weight 400 although their ancestors compute 600; its native
  Checkout title is pale rather than cyan. Merman retains its requested edge effects,
  explicit 600 label weight and cyan title. These differences prevent whole-image equivalence;
  they are not reasons to remove the typed behavior.
- Sequence retains four participant boxes, two glowing lifelines, distinct solid/dashed
  messages and cyan bound arrowheads, the translucent activation, magenta note, loop frame
  and polygon keyword box. Actor/loop/note glyphs have their separate effects; message glyphs
  correctly have none. The actual leaf weights are 400 in both engines. The reference note
  leaf is dark gray despite its magenta parent; Merman retains the already documented readable
  magenta adaptation. These observations confirm the earlier 11.12.1 text findings on 11.17.2.
- XY retains eight translucent bars with opaque 2px strokes, two 3px glowing lines, 18 glowing
  text terminals and 15 cyan ticks at opacity .3. Declaration order remains bar/line/bar/line.
  The fourth reference series lacks an explicit recipe rule and is a pale 2px line without
  glow; Merman deliberately cycles to teal with 3px stroke and glow. The reference's opaque
  internal XY background covers the host grid inside the plot; Merman keeps its self-contained
  grid visible. Both differences remain explicit.

The local observation check passes all four scene/mode rows: visible positive-area glyphs,
actual custom browser fonts, requested geometry/paint/effect bindings and referenced marker
colors. Browser screenshots, native PNG and actual PDF rasterizations were inspected for the
complete fixed scenes; no missing label, arrowhead or visibly cropped glow was found in that
inspection. This is bounded human inspection, not a calibrated pixel tolerance or exhaustive
clip proof. Existing terminal/containment tests remain separate regression evidence.

## Public Web entry point

The built playground was also exercised through its public preset selector and all four fixed
scene/mode inputs, using the full WASM transaction `58420c448ba4`; its source-input freshness
check passed. Each scene was loaded in a fresh navigation and its source-specific text checked
before observing output. Flowchart HTML/native each contain 13 effect applications, Sequence
21, and XY Chart 28. All four contain three canvas layers and produced no browser page errors.
These captures use host Trebuchet measurement/fonts, independently of the controlled Arial
experiment; they do not establish an embedded-only Web guarantee.

An initial capture harness reused hash navigation and read stale Flowchart output. That result
is retained in `public-ui-stale-navigation.json` and excluded from the evidence above. The
corrected observations and four screenshots are in `public-ui-observations.json` and
`*-public-ui.png`. No production behavior was changed to correct the harness.

## Artifacts and remaining acceptance work

Everything is under `target/bench/experiments/cyberpunk-full-scenes-a40c505df/`:

- `capture-contract.json`, `public.recipe.json`, `caller.recipe.json`, `caller-fonts.json`;
- `capture.rs`, `capture-host-html.rs`, `capture.cjs`, `public-ui.cjs`, `rasterize-pdf.py`;
- `native-controlled.log`, `native-host-html.log`, `browser-observations.json`,
  `pdfium-observations.json`, `structural-checks.json`;
- `comparison.html`, raw SVG, browser screenshots, native PNG/PDF and PDF rasterizations;
- `artifact-hashes.json` for the captured files.

The ignored artifacts contain local font inputs and are not release assets. The capture sources
and logs record the font/profile split rather than silently omitting the failed HTML profile.
No production implementation, fixture expectation, public schema, qualification threshold or
budget changed in this increment; no new unit test is warranted for this observation-only work.

Next, connect complete-scene acceptance to the existing qualification owner, retaining explicit
profile/font scope and target-specific visible assertions. The old palette-only Cyberpunk
qualification remains intentionally invalid. Calibrated terminal-region tolerances, installed
consumer/profile coverage, broader preset usability and U10 size/performance evidence remain
open. Successful native admission alone cannot close those gates.

The independent review identified the HTML projection distinction above, but its agent later
terminated with a session compaction error. That partial review is not recorded as a completed
review or a passing gate.
