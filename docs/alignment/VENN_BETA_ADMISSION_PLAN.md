# Venn Beta Admission Plan (Mermaid@11.16.1)

Status: Admitted
Last updated: 2026-07-22
Pinned Mermaid commit: `7ecca0cd7f1658ef74f4e7e91f925724ef403bbf`

This document records the source-backed plan and admission evidence for `venn-beta` in `merman`.

## Problem

Mermaid 11.16 retains `venn-beta`. Local support now has a source-backed Rust layout kernel, a core detector/parser/typed semantic model, classic and deterministic hand-drawn Stage B SVG rendering, normalized Venn fixtures, committed upstream SVG baselines, and a family-local compare gate with an explicit RoughJS residual boundary.

The parser and DB are small enough to port directly. The risky part is layout/rendering: Mermaid delegates circle placement and intersection geometry to `@upsetjs/venn.js@2.0.0`, then mutates the generated SVG with D3 and optionally replaces shapes with RoughJS for `look: "handDrawn"`. A local renderer must not approximate that geometry with ad hoc circle formulas and then claim Mermaid parity.

## Implementation Progress

- Done: source-backed `@upsetjs/venn.js@2.0.0` / `fmin@0.0.4` Rust layout kernel in `merman-render`.
- Done: core `venn-beta` detector, semantic JSON parser, and typed `RenderSemanticModel::Venn` model.
- Done: classic Stage B SVG renderer foundation for title, circles, intersections, text-node `foreignObject`, debug text-node layout, typed render-model path, and semantic JSON path.
- Done: targeted `xtask compare-venn-svgs`, `gen-upstream-svgs --diagram venn`, and `check-upstream-svgs --diagram venn` tooling.
- Done: normalized Venn fixtures and committed upstream SVG baselines for Mermaid syntax-doc examples.
- Done: `venn` is admitted to `supported_diagrams()` and the primary SVG matrix for classic SVG output.
- Done: Venn renderer theme roles are projected through `PresentationTheme::venn()` for classic SVG output.
- Done: `look: "handDrawn"` emits deterministic seeded `roughr` groups for circles and intersections, including hachure and cross-hatch fills.
- Done: Mermaid Cypress cases 14, 15, and 18 have semantic/layout goldens and pinned 11.16 SVG baselines. Their exact fixture records are admitted as structure-only evidence because JavaScript RoughJS and Rust `roughr` do not produce path-identical coordinates.
- Done: Mermaid Cypress cases 2, 8, 9, 13, 16, and 17 have semantic/layout goldens and pinned
  11.16 SVG baselines. They close the classic-rendering gaps for three-set geometry, quoted set
  identifiers, dark-theme roles, dense labels/text/styles, synthetic pairwise subsets, and partial
  pairwise subsets.

## Source Evidence

- Detector: `repo-ref/mermaid/packages/mermaid/src/diagrams/venn/vennDetector.ts` accepts `/^\s*venn-beta/` and exposes diagram id `venn`.
- Grammar: `repo-ref/mermaid/packages/mermaid/src/diagrams/venn/parser/venn.jison` supports `set`, `union`, `text`, indented text, `style`, quoted identifiers, bracket labels, and numeric sizes.
- DB/model: `repo-ref/mermaid/packages/mermaid/src/diagrams/venn/vennDB.ts` sorts set identifiers for stable keys, defaults single-set size to `10`, defaults union size to `10 / len^2`, and rejects unknown union identifiers.
- Renderer: `repo-ref/mermaid/packages/mermaid/src/diagrams/venn/vennRenderer.ts` calls `venn.VennDiagram()` for SVG generation and `venn.layout()` for text-node placement.
- Styles: `repo-ref/mermaid/packages/mermaid/src/diagrams/venn/styles.ts` defines title, circle text, intersection text, and text-node font/color CSS.
- Config: `repo-ref/mermaid/packages/mermaid/src/config.type.ts` defines `VennDiagramConfig` with `width`, `height`, `padding`, and `useDebugLayout`; `defaultConfig.ts` wires `defaultConfigJson.venn`.
- Dependency: `repo-ref/mermaid/pnpm-lock.yaml` pins `@upsetjs/venn.js@2.0.0`.
- Tests/docs: `repo-ref/mermaid/packages/mermaid/src/diagrams/venn/parser/venn.spec.ts`, `vennRenderer.spec.ts`, `repo-ref/mermaid/cypress/integration/rendering/venn/venn.spec.ts`, and `repo-ref/mermaid/docs/syntax/venn.md`.

## Layout Source Audit

The pinned `@upsetjs/venn.js@2.0.0` npm tarball publishes the source used for layout and geometry. The audit covered `src/layout.js`, `src/circleintersection.js`, `src/diagram.js`, `src/index.d.ts`, `src/layout.spec.js`, `src/circleintersection.spec.js`, and `src/diagram.spec.js`.

| Area | Source-backed finding | Implementation consequence |
|---|---|---|
| Package surface | `@upsetjs/venn.js@2.0.0` is MIT licensed and ships source. Its source imports `fmin@0.0.4` primitives: `bisect`, `nelderMead`, `conjugateGradient`, `zeros`, `zerosM`, `norm2`, and `scale`. `fmin@0.0.4` is BSD-3-Clause. | A Rust port is viable, but license notices and source attribution must be handled when code is ported. Do not depend on a browser or Node runtime in CLI/FFI packages. |
| Circle layout | `venn()` adds missing pairwise intersections, builds an initial layout, then optimizes all circle centers with `nelderMead` and a loss function over pairwise and higher-order overlaps. | The layout adapter must port the optimization loop, not just the final SVG path generation. |
| Initial layout | `bestInitialLayout()` starts with `greedyLayout()` and tries `constrainedMDSLayout()` for larger inputs (`areas.length >= 8`). MDS uses `conjugateGradient` and `Math.random` restarts. | The Rust layout kernel needs deterministic seeded random support for oracle tests. Greedy-only layout is not enough for admission. |
| Geometry | `circleintersection.js` implements `intersectionArea`, `circleOverlap`, `circleCircleIntersection`, circular segment area, containment, and arc stats. | Port geometry as a focused Rust module and cover it with upstream-derived numeric tests before renderer work. |
| Text and path helpers | `diagram.js` uses `computeTextCentre()` with `nelderMead`, `computeTextCentres()`, `intersectionAreaPath()`, `normalizeSolution()`, and `scaleSolution()`. Its `layout()` helper returns `data`, `text`, `circles`, `arcs`, `path`, and `distinctPath`. | The Rust adapter should expose a typed layout result equivalent to `IVennLayout<T>` so the SVG renderer does not reimplement layout internals. |
| Mermaid renderer use | Mermaid calls `VennDiagram().width(...).height(...)` to create the base D3 SVG, then separately calls `venn.layout(sets, { width, height, padding })` to place Mermaid-specific text nodes. | Renderer parity requires both the D3 DOM shape and the helper layout output. The JS package should remain a comparison oracle, not the production runtime path. |
| Upstream tests | `layout.spec.js` covers greedy layout, distance-from-area, normalization, and disjoint clustering. `circleintersection.spec.js` covers segment/overlap/intersection geometry and random failure regressions. `diagram.spec.js` covers text-centre behavior. | Port these as the first layout/geometry test corpus, then add Mermaid syntax-doc fixtures and upstream SVG baselines. |

## Proposed Solution

Implement `venn-beta` around a source-backed Rust layout kernel that mirrors the relevant `@upsetjs/venn.js@2.0.0` behavior. The npm package may be used by `xtask` or tests as an oracle, but not by the runtime renderer.

```mermaid
flowchart LR
    Source["venn-beta source"] --> Parser["Rust parser + Venn semantic model"]
    Parser --> Layout["Rust Venn layout kernel"]
    Layout --> Svg["Stage B SVG renderer"]
    Theme["PresentationTheme Venn roles"] --> Svg
    Upstream["@upsetjs/venn.js 2.0.0 + fmin 0.0.4 oracle"] --> Layout
    Svg --> Compare["compare-venn-svgs + upstream baselines"]
```

The implementation lane should have these slices:

1. Detector and typed parser: add `venn` detector for `venn-beta`, port parser behavior from `venn.jison`, and create a typed model with subsets, text nodes, style entries, title, accessibility metadata, and effective `venn` config.
2. Parser fixtures: port upstream parser cases for labels, sizes, text nodes, style declarations, quoted identifiers, unknown unions, and invalid `set` / `union` arity.
3. Layout kernel: port the relevant `@upsetjs/venn.js@2.0.0` layout, geometry, text-centre, normalize, scale, path, and minimal `fmin` helper behavior into Rust behind a typed adapter. Seed the random MDS path for deterministic oracle tests.
4. Layout oracle fixtures: generate pinned package outputs for small, overlapping, disjoint, nested, higher-order, and text-node diagrams; compare circles, text centres, paths, and loss within documented tolerances before renderer DOM work.
5. Stage B SVG renderer: emit Mermaid-shaped `.venn-circle`, `.venn-intersection`, `.venn-title`, `.venn-text-nodes`, `.venn-text-area`, and `foreignObject` text-node DOM after layout is source-backed. Classic output uses direct paths; `look: "handDrawn"` replaces circle/intersection paths with deterministic seeded `roughr` groups matching Mermaid's RoughJS branch structure.
6. Theme roles: `PresentationTheme::venn()` owns `venn1..venn8`, `vennTitleTextColor`, `vennSetTextColor`, `primaryColor`, `primaryTextColor`, `textColor`, `titleColor`, `background`, font family, and dark/light readable circle text derivation; diagram `style` entries still override per-area fill, stroke, opacity, width, and text color.
7. Fixture and compare gate: import syntax-doc and parser-source fixtures, generate `fixtures/upstream-svgs/venn`, run `xtask compare-venn-svgs`, and keep the family in the main matrix once family-local structural DOM parity is green.

## Alternatives Considered

| Option | Pros | Cons | Decision |
|---|---|---|---|
| Port `@upsetjs/venn.js@2.0.0` layout logic into Rust | Pure Rust, deterministic, no runtime JS dependency, matches headless architecture | Requires porting small optimizer helpers and maintaining numeric tolerances | Recommended after audit |
| Use a JS/WASM adapter for `@upsetjs/venn.js` during layout | Highest layout fidelity initially | Adds non-Rust runtime dependency, complicates CLI/FFI packaging, weakens headless portability | Acceptable only as a comparison oracle |
| Use a generic Rust optimizer crate instead of porting `fmin` behavior | Reduces local optimizer code | Numeric behavior and termination may drift from the pinned source before SVG parity is measurable | Defer until a source-backed port proves too costly |
| Implement a local approximate circle solver | Fastest to code | Not source-backed, likely DOM/geometry drift, violates admission rubric | Rejected |
| Parse-only `venn-beta` support | Low risk, gives early diagnostics/model access | Users expect visible diagrams; no parity value for preview users | Defer unless a caller explicitly needs parse-only metadata |

## Success Metrics

| Metric | Target | Measurement |
|---|---|---|
| Parser coverage | Upstream parser spec behavior covered by semantic snapshots | `cargo nextest run -p merman-core venn` |
| Geometry parity | Circle intersection, overlap, segment area, and path helpers pass upstream-derived numeric tests | Dedicated Rust layout/geometry tests |
| Layout source parity | Layout adapter outputs match the pinned `@upsetjs/venn.js@2.0.0` oracle for initial fixtures within documented tolerance | Dedicated layout tests or fixture snapshots |
| SVG structural parity | Family-local Venn DOM parity passes for committed upstream baselines | `cargo run -p xtask -- compare-venn-svgs --check-dom --dom-mode parity --dom-decimals 3` |
| Matrix admission | `venn` is admitted to `compare-all-svgs` after detector, parser, layout, renderer, fixtures, and baselines exist | `cargo run -p xtask -- check-alignment` |

## Risks and Mitigations

| Risk | Severity | Likelihood | Mitigation |
|---|---|---:|---|
| Venn layout drift from `@upsetjs/venn.js` | High | High | Port the pinned layout/geometry/text-centre code path first; use pinned package output as an oracle before writing renderer DOM |
| Optimizer behavior drift | High | Medium | Port only the `fmin` primitives used by Venn initially and cover them through layout-level oracle fixtures |
| Random MDS restarts produce unstable higher-order diagrams | Medium | Medium | Add a deterministic seed path for tests, mirroring the repo's existing seeded Mermaid baseline pattern |
| Ported license obligations are missed | Medium | Low | Record MIT/BSD-3-Clause attribution in the implementation PR when source code is ported |
| Browser/D3 serialization noise | Medium | Medium | Normalize only non-semantic D3 wrapper differences in the family compare adapter; do not hide geometry or label differences |
| `foreignObject` text-node parity differs across renderers | Medium | High | Document strict HTML/browser text-metric residuals separately from structural DOM parity |
| RoughJS and `roughr` path coordinates differ | Medium | High | Keep the three exact Cypress hand-drawn fixtures in structure-only DOM evidence, retain path geometry as a visible residual, and never weaken comparison for classic or future fixtures by name/config heuristics |
| Packaging impact from a JS layout dependency | High | Medium | Prefer Rust port; if a JS oracle is used, keep it in test/tooling, not runtime packages |

## Hand-Drawn Evidence Boundary

The admitted hand-drawn corpus is deliberately exact and source-traceable:

- `upstream_cypress_venn_handdrawn_two_set_014` covers two sets and their intersection.
- `upstream_cypress_venn_handdrawn_three_set_title_015` covers three sets, pairwise and three-way intersections, and a title.
- `upstream_cypress_venn_handdrawn_custom_styles_018` covers text nodes and per-set/intersection fill overrides.

Each fixture pins `look: handDrawn`, `handDrawnSeed: 1`, and `fontFamily: courier` from its
Mermaid 11.16 Cypress configuration. The upstream family was rendered twice in the same attested
environment with identical SVG SHA-256 hashes. Local renderer tests separately lock deterministic
DOM order, seed propagation, hachure/cross-hatch fill structure, and transparent Khroma color
serialization.

The comparator maps requested strict/parity comparison to structure mode only for these exact
family-and-stem records. It does not infer a weaker mode from `handDrawn` config or fixture naming.
This proves renderer ownership and stable SVG structure without claiming path-level DOM parity:
JavaScript RoughJS and Rust `roughr` retain observable path-coordinate differences.

## Classic Fixture Coverage

The active classic corpus is intentionally behavior-complete rather than a byte-for-byte copy of
all 18 Cypress inputs. The syntax-doc fixtures already cover the simple two-set, title, labels,
sizes, asymmetric sizes, text-node, area-style, and text-style cases. The six exact Cypress imports
below cover the behavior that those fixtures did not exercise:

| Cypress case | Fixture | Additional behavior |
|---|---|---|
| 2 | `upstream_cypress_venn_spec_2_should_render_a_three_set_venn_diagram_002` | Classic three-set layout with all pairwise and three-way intersections |
| 8 | `upstream_cypress_venn_spec_8_should_render_a_venn_diagram_with_string_identifiers_008` | Quoted identifiers and quoted union references |
| 9 | `upstream_cypress_venn_spec_9_should_render_with_dark_theme_009` | Dark-theme presentation roles on a three-set diagram |
| 13 | `upstream_cypress_venn_spec_13_should_render_a_complex_venn_with_labels_text_nodes_and_style_013` | Three-set labels, 13 text nodes, and set styles in one layout |
| 16 | `upstream_cypress_venn_spec_16_should_render_a_venn_diagram_with_a_3_set_union_without_expli_016` | Pairwise-subset synthesis for a lone three-way union |
| 17 | `upstream_cypress_venn_spec_17_should_render_a_venn_diagram_with_partial_pairwise_subsets_017` | Completion of only the missing pairwise subset |

Together with the three syntax-doc fixtures and three exact hand-drawn Cypress fixtures, this gives
12 active Venn fixtures. Every active fixture has a semantic golden, layout golden, pinned upstream
SVG, and family-local DOM comparison; omitted Cypress inputs add combinations of already-covered
behavior rather than a new parser, layout, theme, or renderer contract.

## Admission Decision

`venn-beta` is admitted for classic SVG output and deterministic hand-drawn SVG output. The
source-backed layout/parser, both renderer branches, normalized fixtures, upstream baselines, and
targeted compare tooling are in place. `@upsetjs/venn.js@2.0.0` remains a test/tooling oracle only,
not a runtime dependency.

Classic fixtures retain full parity comparison. The three pinned Mermaid Cypress hand-drawn
fixtures carry structure-only evidence: they prove the expected rough groups, element ordering,
fill modes, labels, and text-node wrappers, but do not claim JavaScript RoughJS and Rust `roughr`
path coordinates are identical.

The renderer now consumes Venn theme roles through `PresentationTheme::venn()`, so Venn-specific theme fallback chains no longer live inside the SVG emission module.

## Admission Gates

- `cargo nextest run -p merman-core venn`
- dedicated Venn geometry/layout oracle tests
- `cargo nextest run -p merman-render venn`
- `cargo run -p xtask -- compare-venn-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-venn-svgs --filter upstream_cypress_venn_handdrawn --check-dom --dom-mode structure --dom-decimals 3`
- `cargo run -p xtask -- compare-venn-svgs --filter upstream_docs_venn --check-dom --dom-mode parity-root --dom-decimals 3`
- `cargo run -p xtask -- check-upstream-svgs --diagram venn --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- check-alignment`

Latest focused gates for classic and hand-drawn rendering:

- `cargo nextest run -p merman-fixture-render-context --test catalog_contract`
- `cargo nextest run -p merman-render venn_svg presentation_theme`
- `cargo nextest run -p merman-render --test venn_svg_test`
- `cargo check -p merman-render`
- two consecutive `cargo run -p xtask -- gen-upstream-svgs --diagram venn` runs with identical fixture SVG hashes
- `cargo run -p xtask -- check-upstream-svgs --diagram venn --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-venn-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo fmt --all --check`
- `git diff --check`
