# Flowchart rectangle geometry — 2026-09-18

Source baseline: `60ee01e77`. This bounded U6 increment follows the
[theme product plan](../../../plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md).
It does not close U6, qualify the complete Cyberpunk appearance or freeze C7a.

## Production changes

The classic and Neo RoundedRectangle writers now consume the selected typed corner radius and
return the existing rectangle paint/stroke/radius receipt. Both retain their original 5px default
when no configuration, source declaration or typed radius overrides it; explicit zero emits zero.
The Process and RoundedRectangle writers materialize Neo configuration and admitted source rx/ry
into their actual geometry attributes. Source axes remain independent: a classic Process with
only rx leaves ry absent for SVG's automatic mirroring, while a RoundedRectangle retains the
other axis's 5px default. The source declaration keeps precedence over the typed theme.

Selected source and typed radii also reach canonical inline rx/ry declarations. This prevents
Neo's default stylesheet from overriding typed attributes in browsers, and aligns case-insensitive
source winners with native geometry. Existing declaration preparation determines those winners;
no new CSS cascade interpreter was added. Hand-drawn output remains unverified, and radius Clear
retains its existing residual.

The public Cyberpunk complete recipe now declares 3px Flowchart Node strokes and 2px Flowchart
Edge strokes. These are ordinary family-scoped recipe fields, shared by public compile and
export/import. Sequence and other families do not inherit these Flowchart widths. No new public
selector, runtime preset exception, dependency or font was added.

## Public applicability gap remains open

The reference's rx/ry declarations do not round a Diamond polygon. Current `Node.radius`, however,
is a request on every matching Node, and the rule model has no shape filter. Changing every
unverified radius receipt to NotApplicable would hide an unconsumed request. Existing Diamond
paint-plus-radius strict rejection is intentionally unchanged; the public recipe does not yet
request the reference's 10px corners.

A read-only contract review confirmed the same terminal-consumption requirement in Gantt and
Timeline and no existing Flowchart shape selector. U6 must resolve this explicitly: either define
a bounded public corner-channel applicability domain, or offer an explicit shape restriction.
Any solution must survive canonical recipe export/import, retain unknown/unsupported writer
residuals, and distinguish inapplicability from a missing rectangle attribute. Fixture-specific
ordinals and built-in-only behavior are not solutions. The plan records this question without
moving the required corner outcome out of scope.

## Verification

- Before production edits, both targeted tests failed: the rounded rectangle failed strict
  admission, and the public scene lacked its required node stroke width.
- The first expanded renderer run passed 159/160. Its remaining test used a global node-glow
  fixture for Neo, whose effect consumer is still unsupported. Geometry checks now use a
  geometry-only theme for both looks; classic radius-plus-glow is a separate positive assertion.
  The Neo effect boundary was not widened.
- The native test compares 3px versus 1px cyan strokes, and 10px versus zero radius on the same
  rectangle. Its first corner-area check incorrectly used alpha on an opaque default background:
  both alpha counts were 180880 while 3058 pixels differed. The final fixture declares a black
  canvas and white node fill and checks the actual white corner area at 4x scale. Stroke count
  and corner-area thresholds were not lowered.
- Browser checks on actual Renderer SVGs reproduced a separate Neo defect: attributes 10/0
  both computed to 5px. The native typed-radius test passed before that browser fix because the
  native reader ignores CSS rx/ry. A new native source/config equivalence check then failed for
  source radius on classic Process and configured radius on Neo RoundedRectangle. Both required
  geometry attributes in addition to browser CSS.
- A subsequent review found that differently cased source declarations could disagree with the
  prepared semantic winner. Tests now include `rx:4px!important,RX:6px` and reordered duplicate
  spellings. Rectangle writers emit the already-selected winner in both attributes and CSS.
- Final ordinary CI command (`svg,png`, private acceptance cfg): 17/17 passed, 0 skipped,
  including authoring, marker pixels and geometry pixels. The workflow explicitly includes the
  new geometry integration target.
- The final three native geometry tests passed, including the mixed-case source winner. Chromium
  151.0.7922.34 then checked their 24 actual Renderer SVGs: every rx/ry attribute agreed with
  computed style, including classic/Neo, typed 10/0, source 4 and configured 4. Capture logging
  was removed from the test afterward. Results: `/tmp/merman-corners-browser-final.json`.
- The expanded renderer run passed 2836/2837 with two skipped tests. Its single failure
  was an old assertion that source rx/ry attributes must be absent. That assertion now requires
  the actual source values 4/6; its source ownership, typed NotApplicable and residual checks
  remain intact. The final renderer run passed 2837/2837, with two skipped tests.
- Private C6 runtime and preset qualification integrations: 4/4 passed, 0 skipped. The selected
  Flowchart qualification mutation tests passed 4/4; 118 unrelated library tests were filtered
  out. This verifies the existing qualified scope, not complete reference-theme appearance.
- Clippy completed successfully and reported 160 warnings. This is not a zero-warning result.
  `cargo fmt --all -- --check` and `git diff --check` passed.

No complete browser/PDF scene comparison, installed package rebuild or new performance/size
measurement is claimed. U6 still needs a public corner-applicability contract, text weight/glow,
edge glow and the complete three-target visual acceptance.

## Review

The three simplification lenses found no necessary change. Mini-model routing failed; inherited
model reviewers completed quality and efficiency, and the existing applicability reviewer
completed the reuse lens after a new-thread limit. These were read-only passes. The existing
non-hand-drawn path allocation in the RoundedRectangle helper was noted but left outside this
behavioral change; no measured performance improvement is claimed.

Code review run `20260918-120627-geometry` identified the browser CSS override, native
source/config geometry gap, mixed-case source winner mismatch and stale Cyberpunk catalog recipe
fingerprint. All have been addressed. The completed review receipt reports no remaining actionable
findings, with final execution checks delegated to the implementing agent. Thread capacity prevented
independent persona agents; the reviewer performed six lenses directly. This is degraded review
coverage, not six independent reviews. The updated fingerprint comes from the actual canonical
recipe; unpublished schema/revision numbers stay 1.

## Reproduction

All build commands run serially with `CARGO_BUILD_JOBS=1` in the shared target directory.

```console
cargo nextest run --locked -p merman-render --release --no-default-features \
  --features layout-cytoscape --lib --test flowchart_node_effects \
  --test flowchart_marker_theme --test flowchart_svg_test --test theme_materializer_test \
  --no-fail-fast
python3 scripts/run_theme_acceptance.py nextest run --locked -p merman \
  --no-default-features --features svg,png --test theme_authoring \
  --test theme_flowchart_markers --test theme_flowchart_geometry --cargo-quiet
python3 scripts/run_theme_acceptance.py nextest run --locked -p merman-theme-acceptance \
  --no-default-features --features png,layout-cytoscape \
  --test c6_runtime --test preset_qualification --cargo-quiet
python3 scripts/run_theme_acceptance.py nextest run --locked -p merman-theme-acceptance \
  --no-default-features --features png,layout-cytoscape --lib \
  -E 'test(preset_qualification::flowchart_proof)' --cargo-quiet
cargo clippy --locked -p merman-render --release --no-default-features \
  --features layout-cytoscape --lib
```

Final logs use `/tmp/merman-flowchart-corners-{final-green,native-final,qualification,mutations,clippy}.log`.
Earlier `red`, `source-red`, `config-red`, `render`, `pixel-diagnostic` and `final` logs preserve
failed observations. `svg-final` contains the final temporary capture; browser inputs and results
are under `/tmp/merman-corners-browser-final` and `/tmp/merman-corners-browser-final.json`.
These local artifacts are not release-host evidence.
