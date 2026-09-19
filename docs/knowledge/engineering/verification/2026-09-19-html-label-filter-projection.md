# HTML label filter projection

Date: 2026-09-19. Base: `299d973b5`. Status: scoped validation complete.
Scope: the native HTML projection prerequisite of U6 in
`docs/plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md`.

## Observed boundary

Native export uses the requested label mode; it does not automatically render Flowchart with
`htmlLabels=false`. Adding CSS `text-shadow` to a span therefore loses the shadow during the
existing HTML-to-SVG text fallback. The shared effect receipt also requires an actual SVG filter,
so a CSS-only declaration cannot be substituted for a verified native effect.

A Chromium 151.0.7922.34 probe confirmed that an SVG ancestor filter affects an HTML label's
painted output. The native fallback, however, used to append generated text at the SVG root:
the text lost the ancestor filter, its local coordinates, and its painter order. The new regression
failed on the original implementation with `filter was lost`.

## Change

The fallback can keep text under explicit `g` filter ancestors, alongside its original
foreignObject, using local coordinates and the existing transform/filter chain. Labels outside
such groups retain the root-overlay path.

Original-location projection first checks the actual generated group, text and background
attributes against the existing bounded CSS matcher. Matching declarations that can change
unprotected paint, matching `!important`, unadmitted selectors, conditional at-rules and values
outside the source parser's supported grammar decline projection. Static keyframes and custom
property-only unadmitted selectors do not add generated-node paint rules. This does not implement
additional CSS semantics. When projection is declined, the readable root fallback retains its
original placement through the accumulated translation. Missing filtered glyphs remain observable
by the existing native receipt check; a CSS declaration is not substituted for a filter receipt.

The first implementation tried inline priority to preserve measured typography. Native usvg
regression showed that a stylesheet `!important` still changed the 16px text to 10px. Independent
review also found that newly matching SVG selectors could hide the generated text. The final
implementation conservatively checks actual matches instead of claiming inline priority isolates
them. Generated text explicitly carries the resolved font/fill and suppresses SVG-only strokes.

The existing output preflight accounts cumulatively for in-place and root-overlay bytes,
generated elements, and nested generated depth. No dependency, font asset, theme schema,
public option, or native evidence rule changes.

This is a projection fix, not the Flowchart HTML effect consumer. NodeLabel and EdgeLabel
still need the corresponding family preparation/emission work. In particular, an edge-label
background must remain outside a glyph-only filter. This change does not qualify the public
Cyberpunk scene or close U6, U9, or C7a.

## Verification

- Red regression: the generated text escaped the filtered group and was translated to root coordinates.
- First renderer fallback run: 53/53 Release tests passed. The final resource-accounting test was
  added while this binary compiled and requires the subsequent final run.
- Initial native run exposed the priority issue above and rejected a hand-written filter fixture
  missing the writer's explicit color space and SourceGraphic input. The fixture now uses that
  production grammar; exporter validation was not relaxed.
- Final renderer/facade Release regression: 2,629/2,629 passed, two skips. It includes
  cumulative inline/root byte accounting and depth rejection, original-coordinate/painter-order
  preservation, generated-node CSS negatives, and the existing native Flowchart effect regressions.
- Native PNG contains actual pink shadow pixels. PNG and PDF both report one observed filter
  application/reference for the converted glyphs. The positive fixture includes ordinary generated
  text CSS, keyframes and a root custom property; unknown/conditional CSS has separate negatives.
- A final standalone native fixture recheck passed 8/8 on the same source, without embedded-fonts.
- Scoped Clippy, `cargo fmt --all -- --check`, and `git diff --check`: passed. Existing dead-code
  and large-error warnings remain; no broad warning cleanup is claimed.
- Independent source review completed with no retained findings after the CSS guard corrections.
  Seven review lenses ran serially in one separate Codex context because additional agent slots
  were unavailable; these were not seven independent reviewers. Parent validation above closes
  the review's pending runtime gate for this increment. Review artifacts are under
  `/tmp/compound-engineering-501/ce-code-review/20260919-171235-html-filter/`.

Local artifacts: `/tmp/flowchart-html-filter-probe.{mjs,json,png}`,
`/tmp/flowchart-html-filter-fallback-red.log`, `/tmp/flowchart-html-filter-fallback-green.log`,
`/tmp/flowchart-html-filter-native.log`, and `/tmp/flowchart-html-filter-simplification.md`.
The independent source investigation is `/tmp/flowchart-html-export-boundary-20260919.md`.
These local artifacts are development evidence, not installed-package or release receipts.


Commands (Cargo runs were serial, `CARGO_BUILD_JOBS=1`, shared target):

```text
cargo nextest run --release --locked -p merman -p merman-render --no-default-features --features merman/svg,merman/png,merman/pdf,merman/layout-cytoscape --lib --test resvg_safe_typography --test theme_flowchart_edge_effects --test-threads 2 --no-fail-fast
cargo nextest run --release --locked -p merman --no-default-features --features svg,png,pdf,layout-cytoscape --test resvg_safe_typography --test-threads 1
cargo clippy --locked -p merman -p merman-render --no-default-features --features merman/svg,merman/png,merman/pdf,merman/layout-cytoscape --lib --test resvg_safe_typography --test theme_flowchart_edge_effects
```

Final logs: `/tmp/flowchart-html-filter-final.log`,
`/tmp/flowchart-html-filter-fixture-final.log`, and `/tmp/flowchart-html-filter-clippy.log`. No complete workspace, browser matrix, all-platform
build, public-preset qualification, package-size or performance comparison was run for this
projection increment. The browser probe establishes the chosen SVG filter mechanism only;
it is not a freshly rebuilt public preset witness.
