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

The machine-readable matrix is `fixtures/themes/manifest.json`, backed by the manually curated,
hash-bound review record in
`fixtures/themes/evidence/modern-mermaid-theme-mechanisms.json`. The snapshot owns the reviewed
mechanisms and value-level facets. The manifest only binds typed fixture consumers and a sparse
default-plus-override portability policy. The fixture crate derives the closed Browser SVG,
standalone SVG, PNG, JPEG, and PDF contracts instead of accepting five copied capability lists.
Each theme record retains its reviewed `themes.ts` declaration line and effective canvas image
layer count. A canvas gradient is valid exactly when that count is nonzero, and
`canvas-layering` is valid exactly when more than one image layer is composed. Ordinary catalog
loading checks a closed semantic-matrix digest to make every interpretation change an explicit
code-review event.

Merman does not parse or execute `modern_mermaid` TypeScript and does not implement a partial CSS
background parser for this corpus. `modern_mermaid` is a low-frequency design reference, not a
supported input language. Maintainers manually decompose a pinned theme into Merman-owned typed
theme parameters, update the declaration-line citation and semantic digest, and review the
resulting fixture coverage. The pinned checkout gate independently verifies revision, license,
source hashes, and citation bounds; it intentionally does not pretend to infer browser semantics.
`Portable` in this corpus means the intended admitted semantics and retained resources can be
equivalent for that target. It does not promise identical font raster pixels, browser `getBBox()`
floats, or that the implementation already supports the theme as a public preset.

The mechanism matrix translates selector-heavy source techniques into required semantic owners.
For example, a source `:nth-child()` palette records both the source selector evidence and the
required stable ordinal-palette capability. The portable public spec will expose the semantic
owner, not CSS selector strings. The raw selector evidence stays in the
`fixture-style-precedence` source-compatibility fixture and the trusted compatibility lane. It is
not eligible to prove target capabilities. Typed consumers are split by semantic responsibility:
State relational and negation rules, Class relational rules, ER relational rules, and ordinal
palettes have separate fixtures. Every typed fixture has a hash-bound input under
`fixtures/themes/inputs`; the fixture crate derives mechanisms, value-level facets, visible source
text, target outputs, and per-target capabilities from the structured input, parsed Mermaid source,
and translation table. Expectation JSON carries only evidence metadata that cannot be derived.

Aurora retains the named backdrop-blur residual for every target until a source-backed portable
representation is admitted. The other 23 themes are modeled as portable. Glassmorphism only
mentions backdrop blur in a source comment and explicitly simulates the effect with drop shadows,
so it does not inherit that residual. Win95 contains dash and relational/negation selector
mechanisms across State, Class, and ER diagram families, but no animation or `@keyframes` in the
pinned source. Cyberpunk's canvas contract is
recorded as `screen` blending over a tiled linear grid plus a non-repeating radial glow, matching
`bgStyle` rather than reducing the theme to generic gradient support. The corpus records layered
canvas composition separately from pattern semantics: multiple non-repeating gradients require
`canvas-layering`, while only repeating or explicitly tiled gradients require `canvas-pattern`.
Dotted radial tiles and repeating linear stripes also have separate fixtures so those source values
cannot substitute for one another. These are source-backed boundaries rather than comparator
normalization. Linear Light
includes the cluster `stroke-dasharray` from the pinned source and does not claim rounded geometry;
this distinction is locked into the matrix rather than inferred from the theme name or neighboring
presets.

Canvas evidence composes `bgClass` and `bgStyle` because Preview applies both to the same container.
Consequently, a solid class background remains part of the canvas contract even when one or more
inline gradients or patterns are layered above it.

## Licensed Font Tranche

The corpus vendors two minimum WOFF2 slices from Excalidraw commit
`e4ab626739f5f163c5eca56190f615643218b61c`:

- Excalifont Basic Latin, SHA-256
  `e4423b318e11432aff2e6e865e300b7ca270f92321a3f56632268fede01c1b48`.
- Xiaolai upstream WOFF2 asset whose declared range includes `U+6D4B` and `U+8BD5`, SHA-256
  `6f78f52b32b85a686bae2602b6f4bd9197e508140a87c23eb38ced0fa501808e`.

Both assets carry the SIL Open Font License 1.1 and separate provenance notices. Excalidraw project
code remains bound to its MIT license independently from the font license. The mixed-script fixture
uses only Basic Latin plus `测试`; catalog validation parses the Mermaid model, extracts visible
labels, and checks their codepoints against the union of declared font ranges. Font families must
resolve to the declared asset metadata, and every declared font asset must be selected by the
fixture stack.

## Ownership Boundary

`annotationColors` and the interactive annotation overlay remain application-owned. They are not
copied into the fixture specs or treated as core diagram-theme roles. The corpus admits only diagram
semantics, canvas layers, licensed resources, and explicitly named compatibility residuals.

The publish-false `merman-theme-fixtures` crate validates normalized paths, source and copied-asset
SHA-256 values, hashed licenses/notices, pinned source snapshots, unique IDs, reference integrity,
the exact 24-theme set, explicit source-to-capability translations, same-mechanism typed fixture
evidence for every target, value-level source facet coverage, and target-local portability
residuals. Mermaid style precedence is bound
to its own pinned source snapshot, including ClassDiagram's asymmetric encounter-order behavior and
the two opposing source fixtures. Class encounter-order evidence comes from parser-observed,
target-specific `cssClass` and `classDef` application witnesses rather than final-style equality or
line-text guesses. Flowchart compatibility evidence admits the trusted visual directive keys into
the fixture engine's effective config, parses qualified CSS rules and non-empty declarations, and
rejects selector/property words that exist only in comments, strings, or invalid placeholders.
The 25-entry Mermaid precedence snapshot is closed over parser-backed Flowchart, Class, State, and
ER consumers. Typed capability fixtures reject source-owned visual evidence, every precedence
entry has a source-compatibility fixture consumer, and every hashed Mermaid evidence file is cited
by a matrix entry. Sequence remains documented but is not admitted until its fixture parser and
consumer exist.
`ThemeFixtureCatalog::verify_source_checkout` independently checks the pinned revision attestation,
upstream license, evidence files, cited Mermaid line-range bounds, each modern theme declaration
line bound, and source font bytes against a supplied local checkout. The reviewed snapshot and its
semantic digest own the manual theme decomposition; checkout verification does not evaluate the
upstream TypeScript or CSS. The fixture crate deliberately does not define the eventual public
`DiagramThemeSpec` JSON contract.

The local checkout gate is intentionally ignored in ordinary CI because `repo-ref` is not a
workspace input. Run it explicitly when refreshing the corpus:

```bash
MERMAN_REPO_REF_ROOT=repo-ref cargo nextest run -p merman-theme-fixtures \
  --run-ignored ignored-only pinned_source_checkouts_match_every_manifest_hash
```
