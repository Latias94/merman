# Flowchart radius applicability — 2026-09-18

Implementation baseline: `61d795f41`; final regression baseline after the main merge:
`f63dda087`. This U6 increment follows the
[product plan](../../../plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md)
and the [rectangle geometry fix](2026-09-18-flowchart-rectangle-geometry.md).
It does not close U6/R1 or C7a.

## Contract decision

Modern Mermaid `a021cbce37fc0b07a9f4791c28e983101ea06f2d`,
`src/utils/themes.ts:235`, applies rx/ry 10 to node rectangles, circles, polygons and paths.
The declaration does not round polygon vertices. Flowchart/Swimlane Node numeric radius therefore
configures an existing corner channel; it does not replace the semantic shape with a new path.

Only the concrete Diamond polygon writer now issues an explicit NotApplicable radius receipt.
The default/unverified receipt and missing rectangle consumption do not issue that fact. The
recorder skips only an admitted numeric candidate, retaining unsupported Clear and independently
checked source CSS residuals. Sibling fill/stroke can apply without claiming RoundedGeometry;
a sibling residual or any other unverified occurrence still prevents complete application.

This is a family/target contract. Quadrant ChartSeries radius remains point size, and other
families retain their existing Clear behavior. Either source rx/ry axis currently supersedes the
scalar typed corner value; the other axis retains the shape/configuration or SVG automatic-radius
behavior. Discovery remains Conditional because its query has no shape occurrences.

A public shape selector would require a new classification, matching and exchange contract for
all authors. It is unnecessary to express this property domain. No new selector, wire field,
version, dependency or proof framework was added. The ordinary Cyberpunk Flowchart Node rule now
contains radius 10; serialized recipes preserve the same rule without ordinal expansion or
preset-specific writer dispatch. The canonical recipe fingerprint was updated from actual output.

## Verification

- Numeric Diamond and mixed-shape radius tests failed strict admission before the change and
  pass after it. The public recipe test separately failed because rectangle rx/ry were absent.
- Integration cases cover radius-only Diamond, mixed rectangles/Diamond, merged versus split
  paint/radius rules, source CSS residuals, Clear, hand-drawn output, unverified Circle and an
  unsupported sibling facet. Recorder tests preserve missing-writer residuals in both ordering
  directions and check that inapplicable radius adds no RoundedGeometry capability.
- The public recipe is exported/imported through the regular interface; its three U1 rectangles
  must have rx/ry 10 while the Diamond stays a polygon.
- Native public-recipe comparison changes only radius 10 to zero: rectangle-scene pixels must
  change, while Diamond-only pixels must remain identical. The first 4x run passed in 60.904s.
  This focused contrast now runs at 1x and passed in 4.602s with the same >100 changed-pixel
  threshold and exact Diamond pixel equality. Existing geometric controls and qualification
  retain their 4x scale. This is a test-fixture cost comparison, not a runtime performance claim.
- The native authoring/marker/geometry CI command passed 18/18, 0 skipped, before
  the main merge and again on `f63dda087` plus this increment. The latter run completed
  in 4.769s; no production performance comparison is inferred from this test timing.
- Chromium 151.0.7922.34 checked four actual public-recipe/control SVGs. In the U1 scene the
  three rectangles had matching rx/ry attributes and computed values of 10px (or zero in the
  control), 3px borders, and one unchanged Diamond polygon. Diamond-only controls had no rx/ry.
  Results and the current scene screenshot are under `/tmp/merman-radius-domain-browser`.
  Temporary capture logging was removed afterward.
- Renderer/discovery Release regression before the merge passed 2,888 tests, two
  skipped. The post-merge run with `math,layout-cytoscape` passed 2,924 tests, two
  skipped, covering library tests and Flowchart node effects, markers, SVG output,
  label measurement, theme resolution and discovery.
- C6 runtime and public preset qualification: 4/4 passed, none skipped.
- Flowchart qualification mutation tests: 4/4 passed; 118 unrelated library tests
  were filtered out. Missing or changed terminals and missing painted surfaces
  remain rejected.
- Scoped renderer Release Clippy with `math,layout-cytoscape` completed successfully
  with 156 warnings; this is not a warning-free build or a dead-code cleanup claim.
- `cargo fmt --all -- --check` and `git diff --check` passed. No temporary SVG
  capture logging remains in the committed test.

## Review and limits

A separate design review checked family-specific radius and Clear contracts before implementation.
The three simplification lenses found no necessary changes; existing inherited-model agents were
reused because balanced-model routing had failed. Code review run
`20260918-130049-radius-04f1b7` completed with six independent reviewers and no remaining findings.
It corrected the single-axis documentation and added an isolated Diamond-only hand-drawn negative
case, so rectangle residuals cannot mask a rough Diamond regression. Final execution checks remain
separate from that completed static review. The reviewed `--unified=10` patch was
compared byte-for-byte with the post-merge implementation and matched; the final
record updates only execution results.

The public recipe still needs the planned text/edge effects and complete SVG/PNG/PDF scene
acceptance. No installed-platform rebuild or new performance/size measurement is claimed.

Logs use `/tmp/merman-radius-domain-{red,preset-red,focused,native,final,qualification,mutations,postmerge-native,clippy}.log`.
The post-merge renderer run is `/tmp/merman-main-sep18-render.log`.
The first focused run passed 19/20, with only the expected catalog fingerprint drift; the actual
compiled value is `ac8ad7060e9a739277c30ba1c66c0f7af5f1e0f495b568e7ff75959d213763e9`.
