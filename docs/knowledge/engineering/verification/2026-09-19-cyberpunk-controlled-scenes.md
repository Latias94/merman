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

## Recovery revalidation

On resuming from `57b0f5d6d`, the ignored capture directory and its observation-check script
were absent. The captures above remain historical observations; this continuation did not
rerun their browser, controlled-font or PDF checks.

The existing preset admission owner now reads the three checked-in complete scenes directly
for Cyberpunk, using the unmodified public recipe, native SVG labels, system fonts, default
limits and 1x PNG. Its fresh six artifact observations are HostDependent with no theme, source,
bridge or Mermaid residuals. Other presets retain their earlier admission scenes.

The first fresh regression also exposed a stale Flowchart qualifier: it expected the old 0.5
fade despite the typed background alpha correction in `a40c505df`. The qualifier now checks
opacity 1 and the actual authored opaque RGB. Negative controls reject both the old fade and
missing background while preserving labels. The Cyberpunk palette-only qualifier still rejects
the current recipe; no qualified cells were promoted.

Release nextest passed all 10 selected qualification tests, with 114 unrelated library tests
filtered out. This command uses the existing acceptance configuration and serial build/test jobs:

```console
CARGO_BUILD_JOBS=1 python3 scripts/run_theme_acceptance.py nextest run --release --locked -p merman-theme-acceptance --no-default-features --features png,layout-cytoscape --lib --test preset_qualification -E 'test(preset_qualification::) | binary(preset_qualification)' --test-threads 1 --no-fail-fast
```

This closes the full-scene admission regression and stale background oracle only. Calibrated
visual qualification, PDF/HTML acceptance, installed consumers and C7a remain open.

## Executable public-browser scene checks

The continuation from `371e5aad2` replaces the two-node Playground probes with the unchanged
checked-in complete fixtures. `playground/tests/theme-cyberpunk.spec.ts` now exercises four
fresh navigations through the public Web preset selection: Flowchart native text, Flowchart
HTML, Sequence and XY Chart. Both Web source-input and Playground distribution freshness
checks passed for the existing full WASM transaction `58420c448ba4`.

Chromium `151.0.7922.34`, Playwright `1.62.1`, browser measurement and the host Trebuchet profile
were used. These checks do not rerun the controlled Arial or native PDF captures above.

| Scene | Nonempty labels | Individually disabled effects | Individually disabled arrowheads |
| --- | ---: | ---: | ---: |
| Flowchart native text | 7 | 13 | 3 |
| Flowchart HTML | 7 | 13 | 3 |
| Sequence | 9 | 21 | 2 |
| XY Chart | 18 | 28 | 0 |

Every expected label has positive bounds within the SVG viewport, the expected actual text-leaf
color, and a pixel contribution inside its own bounds when compared with the hidden label.
Every effect and bound arrowhead independently changes decoded RGBA pixels when disabled.
All four scenes also assert the navy base, three ordered screen-blended layers, radial geometry,
gradient stops and 40-unit grid. Existing Flowchart glow radii, HTML text weight and label
background separation assertions remain in place.

The probes cover both filter attributes and inline filter styles. Disabling only an attribute
would leave the Sequence frame's inline filter active; HTML edge-label effects have inline
styles without filter attributes. Each screenshot first forces a complete SVG layout/repaint.
Without that control, incremental Chromium repaint changed a small set of restored Flowchart
edge-label pixels by up to 35 channel values. With the shared repaint path, restoring the
original styles restores every decoded pixel exactly. No pixel tolerance was introduced.
The initial probe failures were corrected in the test harness, with no renderer change.

All four browser tests passed with one worker. Browser TypeScript checking, scoped ESLint and
`git diff --check` also passed. Reproduce the browser gate with:

```console
node platforms/web/scripts/verify-wasm-inputs.mjs --package full
node playground/scripts/verify-dist-wasm.mjs
npm --prefix playground/tests run test:desktop -- theme-cyberpunk.spec.ts
npm --prefix playground run test:browser:typecheck
```

The tests save scene screenshots in their Playwright output directories. They establish local
pixel contributions on this browser/profile, not whole-image equivalence with Mermaid, complete
glyph/glow clip coverage, native PNG/PDF qualification or portable fonts. The old Cyberpunk
palette qualifier still rejects the current recipe; catalog cells, U6–U9 and C7a remain open.

## Executable native PNG scene checks

The continuation from `48a618a2e` adds `cyberpunk_public_preset` to the existing acceptance
crate. Its three tests render the unchanged complete fixtures through the public facade with
the resource-free Cyberpunk preset, native SVG labels, system fonts, 1x PNG and default limits.
Each preset is also exported to JSON, imported through a fresh compiler and rendered again;
recipe fingerprints, SVG bytes and every decoded PNG pixel match. PNG admission remains
HostDependent with only SystemOrHostFontDependency, and the actual native filter receipts retain
13/21/28 references for Flowchart/Sequence/XY respectively.

The test's mutation path goes through the existing ResvgSafe finalizer and native PNG exporter.
It must reproduce the facade's original pixels exactly before any negative probe runs. XML
element/attribute ranges come from the workspace's existing roxmltree version, added only as an
acceptance dev-dependency; no SVG parser, renderer or qualification framework is introduced.

Across the scenes, 110 independent removals must change actual RGBA pixels: 34 nonempty labels,
62 effects, five bound arrowheads and nine canvas layers. Label probes first disable every
effect, so glow cannot stand in for missing glyph ink. Effect probes override inline styles as
well as presentation attributes; marker probes change the marker attribute because ResvgSafe
deliberately rejects CSS marker declarations. Each mutation starts from its unmodified baseline.

The new three tests and ten existing qualification regressions pass in the serial Release
acceptance profile, with 114 unrelated library tests filtered out. Formatting and diff checks
pass. Scoped Clippy completes without diagnostics in the new test file; `-D warnings` remains
blocked by existing acceptance-library warnings, including the C6 comparison macro, a large
constant fixture array and existing proof-helper style warnings. No warning policy was changed.

```console
CARGO_BUILD_JOBS=1 python3 scripts/run_theme_acceptance.py nextest run --release --locked -p merman-theme-acceptance --no-default-features --features png,layout-cytoscape --lib --test preset_qualification --test cyberpunk_public_preset -E 'test(preset_qualification::) | binary(preset_qualification) | binary(cyberpunk_public_preset)' --test-threads 1 --no-fail-fast
```

This gate proves the named native PNG contributions and recipe exchange under this host profile.
It does not prove full glyph/glow containment, current-source PDF rasterization, controlled-font
portability or reference-image equivalence. Runtime catalog qualification still needs a new
complete-scene semantic contract and profile; the historical Cyberpunk palette qualifier remains
rejecting, and these regression tests do not issue or promote qualification cells.

## Reproducible PDF raster probes

The continuation from `5fb6879db` checks actual PDF pixels through the public CLI. The checked-in
[PDF probe](../../../../tools/debug/check_cyberpunk_pdf.py) owns only the three fixed complete
scenes and their pixel-removal comparisons. It reuses CLI rendering and Python's XML parser;
it does not implement SVG rendering, issue qualification receipts or interpret theme support.

A fresh serial Release CLI build uses defaults plus `layout-elk`. That feature set was checked
against `package.metadata.dist.features` and matches the declared distribution set. The local
macOS ARM64 executable reports `merman-cli 0.8.0-alpha.7`, is 52,336,912 bytes, and has SHA-256
`680c52b61a3675817dc0e64a7a8df1f5c0432584f535feda632a39591413dd9a`. This is a local executable
observation, not an installed/archive qualification or a comparative size result.

The rasterizer is pypdfium2 `5.13.0` with PDFium `153.0.7999.0`, Pillow `12.3.0` and Python
`3.13.15`. Pages are rendered serially at 96 dpi onto opaque white and compared as RGB pixels,
using the [pypdfium2 page-rendering API](https://pypdfium2.readthedocs.io/en/stable/python_api.html).
These tool dependencies remain outside the repository's production dependency closure. Fonts
remain host/system supplied; this run does not reuse the earlier controlled Arial capture.

For each scene, the CLI first renders the original Mermaid source with the public Cyberpunk
preset, `htmlLabels:false` and default PDF options/limits. Its ResvgSafe SVG is then parsed,
serialized and exported with `render --input-kind svg --format pdf`. The resulting PDF raster
must exactly equal the original public-entry PDF raster before mutation probes run.

| Scene | PDF pixels at 96 dpi | Successful individual removals |
| --- | --- | ---: |
| Flowchart | 1110 × 502 | 26 |
| Sequence | 799 × 732 | 35 |
| XY Chart | 1011 × 755 | 49 |

All 110 probes passed: 34 labels with all effects disabled, 62 individual effects, five actual
arrowheads and nine canvas layers. The smallest per-probe peak RGB difference was 213 for
labels, 36 for effects, 164 for arrowheads and 7 for canvas layers. Those are observed values,
not newly accepted tolerances; each probe requires a nonempty exact pixel difference. The three
original PDF rasters were also inspected: expected labels, arrows, note/activation/loop elements,
bars, lines and canvas layers are visible, with no visibly cropped glyph or glow in that bounded
inspection. This is not an exhaustive containment proof or Mermaid whole-image equivalence.

Reproduce with an unused output directory:

```console
CARGO_BUILD_JOBS=1 cargo build --release --locked -p merman-cli --features layout-elk
uv run --python 3.13 --with pypdfium2==5.13.0 --with pillow==12.3.0 python tools/debug/check_cyberpunk_pdf.py --cli target/release/merman-cli --output target/bench/experiments/cyberpunk-pdf-pixels-5fb6879db
```

The output directory contains original and mutated SVG/PDF/raster files, captured CLI capabilities,
the render config and `results.json`. The report records input, script, executable, PDF and pixel
digests; rasterizer/host versions; page dimensions; each difference region; and failure status.
An existing output directory is rejected. The script checks source and executable hashes across
execution, but does not establish clean-build or release-archive provenance by itself.

The real CLI/PDF run, Python syntax check and diff check passed. The full CLI build emitted existing
warnings; no renderer or lint policy changed. Current-source PDF pixel contributions now have a
repeatable checked-in probe. Formal full-scene semantic qualification, catalog/profile binding,
controlled-font portability, installed consumers, cost comparison and C7a remain open.

## Runtime qualification of the complete native scenes

The continuation from `b3cef6784` moves the existing PNG contribution checks into the production
acceptance runner and replaces Cyberpunk's obsolete palette-only qualifier. The declared profile
is `native-cyberpunk-full-scenes-system-fonts-v1`: the unchanged public Flowchart, Sequence and
XY Chart fixtures, native labels, system fonts, default limits, SVG and 1x PNG. It yields six
execution-local **HostDependent** cells. Brutalist and Spotless retain their original six-cell
Flowchart/State/Sequence profile; the shared production catalog remains unqualified.

The fixed scene contract reuses the sealed SVG observer and existing exact writer-declaration
checks. It checks the navy canvas, screen-composited radial/grid layers, family-specific paint,
typography and shape details, label ownership, bound markers, XY series order and glow composition.
Filter-result names are resolved through their actual connections rather than hard-coded internal
names. A positive rename regression and negative geometry, paint, typography, alpha, marker,
label-owner and composition mutations exercise this distinction. Blank artifacts and genuinely
hidden glyphs fail the pixel checks.

All 110 native PNG contribution checks now run before the qualification receipt is constructed;
the separate public-entry tests retain recipe export/import and exact SVG/PNG byte equality.
The acceptance crate reuses the bounded C6 PNG decoder. Its existing roxmltree dependency moves
from test-only to the workspace-only acceptance library; no production package dependency changes.
No general SVG/CSS interpreter or second qualification issuer is introduced.

Connecting the actual CLI exposed a stale Python catalog shape check: the Rust and CLI catalogs
already agreed exactly, including `family_designs`, but the checker rejected that existing field.
The checker now validates and preserves its open family/treatment strings without granting
qualification. Archive and Homebrew test payloads follow the production shape. Profile projection
rejects Cyberpunk's old profile and State scope, and rejects lending its new profile to another preset.

Validation on the existing macOS ARM64/system-font host:

- 15/15 selected Release nextest cases passed, with 114 unrelated cases filtered out.
- 116/116 Python tests passed across qualification, catalog, CLI archive, Homebrew and release-bundle owners.
- The fresh Rust runner and actual full-feature CLI produced identical bytes for all 18 declared
  SVG/PNG observations. Their production catalogs matched; the local projection contains exactly
  six cells per native preset under the appropriate profile.
- Workspace formatting and diff checks passed. Scoped Clippy completed with the same 14 existing
  acceptance-library warnings and no diagnostics in the new scene checker; strict warning-free
  Clippy is not claimed.

The CLI SHA-256 remains `680c52b61a3675817dc0e64a7a8df1f5c0432584f535feda632a39591413dd9a`.
Local runtime reports are `/tmp/merman-full-scene-qualification.json` and
`/tmp/merman-full-scene-cli-qualification.json`; these dirty-worktree observations establish output
agreement, not clean-source or archive provenance. The recording/replay path still requires its
own clean execution. Browser/PDF probes above remain separate evidence, not new cells in this
native SVG/PNG profile. Full clipping containment, controlled-font portability, installed-consumer
coverage, cost comparisons and C7a closure remain open.

## Clean-source recording and replay

After committing the runtime qualifier as `a4a04ef1bba34c98cb452e10f9dad95c66e29e88`, a separate
detached checkout passed the existing clean-tree requirement, including untracked files. It reused
the workspace target directory and ran the unmodified recording command followed by `--check`.
Both completed successfully. The second execution rebuilt/reexecuted the Rust runner, rerendered
all 18 real CLI outputs and matched the entire recorded object, including artifact and receipt
digests, recipe/resource fingerprints, host and both executable identities.

| Identity | Observed value |
| --- | --- |
| Source commit | `a4a04ef1bba34c98cb452e10f9dad95c66e29e88` |
| Cargo.lock SHA-256 | `d2ed8f1b2ed6040b28a9006b89278257b82db6642f6d45ae622ca2990027b081` |
| Qualification executable SHA-256 | `273ee000d112156d4c5e9ebad28eb281d0c70aa83902391a0683b0e7815ee111` |
| Production CLI SHA-256 | `680c52b61a3675817dc0e64a7a8df1f5c0432584f535feda632a39591413dd9a` |
| Qualification record SHA-256 | `49037b24854cb7e83aeb25bab4df08701de9ba7af4c1488371d814bd794e494f` |
| Host | Darwin `25.6.0`, ARM64, system fonts |

The record is stored at
`target/bench/experiments/cyberpunk-qualification-a4a04ef1b/qualification.json`.
Build/replay logs are `/tmp/merman-cyberpunk-clean-{record,replay}.log`; the retained checkout and
absolute artifact paths are in `/tmp/merman-cyberpunk-clean-qualification-context.json`.
Reproduce from that unchanged clean checkout with the same target directory and CLI:

```console
python3 scripts/qualify_theme_presets.py --cli /path/to/merman-cli --output /path/to/qualification.json
python3 scripts/qualify_theme_presets.py --cli /path/to/merman-cli --check /path/to/qualification.json
```

This closes the scoped clean-source recording/replay gap described in the preceding section.
The CLI is the previously built full-feature executable recorded above, now rechecked against
fresh qualified output; it was not rebuilt or extracted from a release archive in this step.
The record has no `cli_archive` provenance and emits no archive companion. It is not the final
same-source candidate or Linux/Windows, installed-consumer, controlled-font or cost evidence.
The shared production catalog remains empty, and the broader C7a/U9/U10 gates remain open.
