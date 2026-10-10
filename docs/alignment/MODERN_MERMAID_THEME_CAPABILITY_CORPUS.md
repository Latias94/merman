# modern_mermaid Theme Capability Corpus

The test corpus in `fixtures/themes` is an architecture probe, not a bundled preset catalog. It is
pinned to `gotoailab/modern_mermaid` commit
`a021cbce37fc0b07a9f4791c28e983101ea06f2d` and records all 24 `ThemeType` entries from
`src/utils/themes.ts`:

```text
linearLight, linearDark, notion, ghibli, spotless, brutalist,
glassmorphism, memphis, softPop, cyberpunk, monochrome, darkMinimal,
wireframe, handDrawn, grafana, noir, material, aurora, win95, doodle,
organic, hightech, kawaii, geometricCollage
```

The historical reference matrix is `fixtures/themes/manifest.json`, backed by the manually
curated, hash-bound review record in
`fixtures/themes/evidence/modern-mermaid-theme-mechanisms.json`. It retains all 24 source theme
records and 18 source-to-semantic mechanism translations, declaration-line citations, value-level
facets, source fixtures, and target-local portability intentions. These records document the
original architecture investigation; they are not executable support evidence or a list of 24
public Merman presets.

Merman does not parse or execute `modern_mermaid` TypeScript or treat its CSS as a supported theme
language. Maintainers manually decompose source techniques into Merman-owned typed theme
parameters. The historical translations distinguish semantic owners such as stable ordinal
palettes, canvas layers, and family-local relational rules from raw selector evidence. Static
expectations and golden records remain attributable reference material; the retired private
catalog no longer parses them or derives target support from them.

The reference records retain the Aurora backdrop-blur residual, Cyberpunk's `screen`-blended tiled
linear grid plus non-repeating radial glow, and the distinction between layered gradients,
dotted radial tiles, and repeating linear stripes. They also retain Mermaid encounter-order and
style-precedence evidence. These source facts remain useful design references, but historical
`Portable` classifications describe intended semantics rather than implementation admission,
artifact qualification, identical font pixels, or public preset availability.

Current support is discovered through the public theme support APIs and documented in
[`diagram-theme-coverage.md`](../rendering/diagram-theme-coverage.md). Public preset qualification
is documented in [`preset-qualification.md`](../rendering/preset-qualification.md). Its live
representative scene inputs are under `fixtures/themes/public-cyberpunk/`; they remain independent
of the historical source catalog.

## Product Portfolio and Family Application

The [September 16 portfolio review](../knowledge/engineering/verification/2026-09-16-theme-portfolio-and-application-boundaries.md) examines all 24 themes and separates shared identity, dedicated family styling, typography-only coverage and host-owned presentation.
Its 72 reference browser captures are compatibility probes, not implementation qualification.
A theme need not be appropriate for every family: use shared defaults plus explicit family recipes, preserve diagram semantics, and report design scope separately from technical support and artifact qualification.
The [current product boundary plan](../plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md) carries those application requirements; Cyberpunk is its first effect-rich scene, not the whole portfolio.

## Licensed Font Tranche

The corpus vendors two minimum WOFF2 slices from Excalidraw commit
`e4ab626739f5f163c5eca56190f615643218b61c`:

- Excalifont Basic Latin, SHA-256
  `e4423b318e11432aff2e6e865e300b7ca270f92321a3f56632268fede01c1b48`.
- Xiaolai upstream WOFF2 asset whose declared range includes `U+6D4B` and `U+8BD5`, SHA-256
  `6f78f52b32b85a686bae2602b6f4bd9197e508140a87c23eb38ced0fa501808e`.

Both assets carry the SIL Open Font License 1.1 and separate provenance notices. Excalidraw project
code remains bound to its MIT license independently from the font license. The manifest retains
font family names, declared ranges, asset hashes, and the mixed-script fixture's Basic Latin plus
`测试` reference. Retaining these licensed reference bytes does not make them available to the
production font resource API or establish current font embedding support.

## Ownership Boundary

`annotationColors` and the interactive annotation overlay remain application-owned. They are not
copied into the fixture specs or treated as core diagram-theme roles. The corpus admits only diagram
semantics, canvas layers, licensed resources, and explicitly named compatibility residuals.

The private source catalog and its alternate CSS/directive parser, derived portability model,
and acceptance-lineage tests have been retired. The workspace continues to own current authoring
fixtures and real SVG/native preset qualification independently. Historical C6 acceptance records
remain under `fixtures/themes/acceptance/` as evidence of the original investigation, not a
current acceptance gate.

When refreshing reference material, review the manifest's pinned source revisions, paths,
SHA-256 values, evidence citations, and license/NOTICE files against the corresponding upstream
checkout. Keep license mirrors and font assets byte-identical, including trailing blank lines.
An updated source snapshot does not by itself change current support or qualification: those
claims require checks at the current public API and emitted-artifact boundaries.
