# Flowchart direction paint — 2026-09-18

Source baseline: `2dd1eab8f`. This record accompanies a bounded U6 implementation under the
[theme product plan](../../../plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md).
It does not close U6, grant public preset qualification or freeze C7a.

## Actual behavior

Flowchart and Swimlane marker definitions now inherit the winning typed Edge stroke, using the
same source/config precedence as the edge path writer. Edge fill fallback and transparent paint
reach the same channels. Point arrows and circle markers receive fill and stroke; cross markers receive stroke.
Circles already have inherited fill, and Neo circles have zero-width outlines, so changing only
their stroke would leave the visible color unchanged. The existing source-only behavior remains unchanged across
classic, Neo and hand-drawn looks. Source and typed variants with identical colors have separate
identities because their paint behavior can differ.

This is an Edge-owned marker color path, not promotion of general `ThemeTarget::Marker` support.
Explicit Marker rules retain their existing residuals. Edge paint Clear also retains the current
unsupported admission result; it cannot silently borrow the fill fallback or become Portable.

The fixed public Cyberpunk Flowchart includes a decision polygon, cluster, Yes/No labels and three
arrowheads. Its first strict test exposed two existing residuals: the Diamond writer painted fill
and stroke but returned the generic unverified receipt. The actual polygon emitter now certifies
its paint, numeric stroke width and dasharray channels. Radius remains unverified because polygon
vertices do not consume `rx`/`ry`. The hand-drawn branch retains its original receipt. No layout or
Diamond SVG geometry is changed.

## Evidence

- Before implementation, actual typed marker color and the fixed public scene tests failed;
  the source/config baseline comparison passed. After marker inheritance landed, the simple
  shape/look matrix passed while the public scene revealed the Diamond evidence gap.
- The source-width/miter regression from the previous tranche now requires strict success for
  numeric Diamond stroke width. Relative CSS widths and unimplemented radius remain negative cases.
- The native pixel test deletes only actual edge marker references, re-finalizes the mutated SVG
  and rasterizes it as a negative control. It compares equal-sized outputs and requires visible
  cyan differences, rather than counting unrelated cyan edges, labels or background pixels.
- The expanded renderer integration run passed 156 tests before the circle correction. The final
  circle change receives an additional marker regression run below.
- Review reproduced a missing circle fill in Chromium: a Neo circle had cyan stroke but inherited
  `rgb(51, 51, 51)` fill and `0px` stroke width. A transparent stroke left that same visible fill.
  Typed circle variants now paint both channels. The first native negative-control run also rejected
  the incomplete output (4 cyan pixels among 348 changed pixels); thresholds were not relaxed.
- The native test now includes visible Neo circles and transparent Neo circles; the transparent
  image must be byte-for-byte equal in decoded pixels after marker references are removed.
- The ordinary theme authoring SVG/PNG CI job explicitly names `theme_flowchart_markers` with
  matching `svg,png` features, so this integration binary cannot pass as an empty cfg-filtered run.
- Final Release native run: 3/3 passed, including State/Flowchart composed-shadow export regression
  and the new marker test's ordinary, Neo colored, Neo transparent and fixed public-preset scenes.
- The first final unit/marker run passed 2692/2694 tests (2 skipped). Two old family tests still
  asserted that typed Edge paint left markers unchanged. Those expectations now inspect the actual
  referenced path's fill and stroke. The explicit Default case also checks exact paint attributes,
  replacing a negative substring check that looked for a CSS declaration inside marker attributes.
- Final Release unit/marker/node-effect run: 2694/2694 passed, with 2 skipped.
- The initial private acceptance run passed 9/10. Its preset pixel check still required the
  historical fixed `#333333` arrow instead of the recipe's edge color. SVG now requires the
  actual referenced arrow's fill and stroke to equal the independently declared preset palette;
  the existing native wing-region check uses that same expected palette without changing its
  region or threshold. New mutations remove or clear each arrow channel independently, alongside
  the existing geometry/reference and pixel-erasure negatives.
- Final private acceptance: 132/132 passed, 0 skipped, including library mutations, the complete
  typed-route SVG/PNG proof and the five named integration binaries. Two existing full proof tests
  were slow; the suite completed in about 347 seconds.
- Exact CI `svg,png` feature/cfg run: 14/14 passed, 0 skipped. Both `theme_authoring` and
  `theme_flowchart_markers` executed; this is local execution of the workflow command, not a remote
  all-host CI result.
- Scoped Release Clippy completed successfully with 160 renderer warnings. This is not a
  warning-free build or a workspace-wide warning cleanup. `cargo fmt --all -- --check` and
  `git diff --check` passed.

## Review and simplification

Reuse and quality simplification reviews found no worthwhile change. Efficiency review identified
one duplicated test XML parse, which was removed, and a small extra typed-color prefix allocation,
which remains: there is no measurement that warrants changing the existing source ID normalizer
for this bounded allocation. No performance improvement is claimed.

The code-review run is `20260918-111445-direction`. Its local artifact directory is
`/tmp/compound-engineering-501/ce-code-review/20260918-111445-direction`. Leaf reviewer dispatch
was unavailable because the thread capacity was occupied and the harness exposed no release tool.
The independent review coordinator completed all selected lenses in one context; those passes
are not independent cross-validation. It found the circle-fill bug above, which was corrected
without changing the existing source-owned marker policy. The follow-up review
`family-tests-supplement.json` checked the three updated family assertions and found no weakened
contract or remaining finding. The `preset-flowchart-proof-supplement.json` review also confirmed
that the qualification expectation follows the actual edge palette without relaxing pixel regions,
thresholds or mutation negatives. Neither supplemental review reran Cargo.

## Remaining U6 work

The public recipe still lacks the reference's 3px node and 2px edge widths and applicable 10px
corners. RoundedRectangle currently has a fixed 5px radius; a broad Node radius request still needs
explicit shape applicability, rather than pretending that Diamond vertices became rounded.
Typed font weight and text/edge glow remain unconsumed. Horizontal/vertical edge glow requires
real user-space filter coordinates and matching native observation; zero-sized object bounds must
not be replaced by fabricated epsilon dimensions. Final line-hop geometry, marker extent and
ancestor clipping must be accounted for by their existing owners.

The visible product gate still requires browser observations, PNG and rasterized PDF for all three
fixed public scenes. This tranche does not establish those gates, final installed artifacts,
performance/size recovery or all-host release evidence. No dependency, font asset, public version
or resource budget was changed.

## Reproduction

Commands are serialized with `CARGO_BUILD_JOBS=1` and use the shared target directory:

```console
cargo nextest run --locked -p merman-render --release --no-default-features \
  --features layout-cytoscape --test flowchart_marker_theme --test flowchart_node_effects \
  --test flowchart_svg_test --no-fail-fast
cargo nextest run --locked -p merman --release --no-default-features \
  --features svg,png,pdf,layout-cytoscape --test theme_flowchart_markers \
  --test theme_composed_effects --no-fail-fast
cargo nextest run --locked -p merman-render --release --no-default-features \
  --features layout-cytoscape --lib --test flowchart_marker_theme \
  --test flowchart_node_effects --no-fail-fast
python3 scripts/run_theme_acceptance.py nextest run --locked \
  -p merman-theme-acceptance --no-default-features --features png,layout-cytoscape --lib \
  --test c6_runtime --test preset_qualification --test block_title_legacy_projection \
  --test class_edge_label_background_legacy_projection \
  --test flowchart_marker_legacy_projection --cargo-quiet --no-fail-fast
python3 scripts/run_theme_acceptance.py nextest run --locked -p merman \
  --no-default-features --features svg,png --test theme_authoring \
  --test theme_flowchart_markers --cargo-quiet
cargo clippy --locked -p merman-render --release --no-default-features \
  --features layout-cytoscape --lib
```

Local logs use `/tmp/merman-flowchart-direction-{final,native-final,lib-final,acceptance,ci,clippy}.log`.
The earlier failing observations are in `/tmp/merman-flowchart-marker-winner-{red,green}.log` and
`/tmp/merman-flowchart-direction-native.log`. These are temporary supporting artifacts; test names,
commands and bounded conclusions are retained here.
