# Cynefin Upstream Test Coverage (Mermaid@11.16.1)

Scope: Mermaid tag `@11.16.1`.

## Upstream Sources

- Parser/integration tests: `repo-ref/mermaid/packages/mermaid/src/diagrams/cynefin/cynefin.integration.spec.ts`
- DB and boundary tests: `repo-ref/mermaid/packages/mermaid/src/diagrams/cynefin/cynefin.spec.ts`
- Renderer: `repo-ref/mermaid/packages/mermaid/src/diagrams/cynefin/cynefinRenderer.ts`
- Boundary helpers: `repo-ref/mermaid/packages/mermaid/src/diagrams/cynefin/cynefinBoundaries.ts`
- Syntax docs: `repo-ref/mermaid/packages/mermaid/src/docs/syntax/cynefin.md`

## Covered Locally

- Domains, empty domains, all five domain names, multiple items, comments, and duplicate-domain
  replacement are covered by parser tests in `crates/merman-core/src/tests/cynefin.rs`.
- Transitions, optional labels, multiple transitions, and self-loop filtering are covered by parser
  tests and semantic fixtures.
- Accessibility/title fields are covered by parser tests and the renderer dispatch test in
  `crates/merman-render/src/lib.rs`.
- Seeded boundary helpers and confusion overflow are covered by unit tests in
  `crates/merman-render/src/cynefin.rs`.
- SVG class/DOM shape for backgrounds, boundaries, cliff, items, arrows, labels, and accessible
  root metadata is covered by `render_model_dispatch_renders_cynefin_svg`.

## Fixture Coverage

- The active corpus contains 13 fixtures:
  - 12 exact Mermaid Cypress render cases from
    `repo-ref/mermaid/cypress/integration/rendering/cynefin/cynefin.spec.js`, covering all five
    domains, transitions, an empty framework, dense domain contents, config sizing/description
    switches, theme variables, straight boundaries, confusion overflow, self-loop filtering,
    accessibility directives, and explicit seeds;
  - `fixtures/cynefin/basic_domains_transitions.mmd`, a local source-backed transition fixture that
    keeps the compact domain/link form in the semantic and layout snapshot lane.
- Every active fixture has a semantic golden, a typed-layout golden, and a pinned Mermaid SVG. The
  Cypress cases are imported from the pinned source; the local fixture is retained because it
  exercises the smallest useful transition graph independently of the larger examples.

## Upstream SVG Baselines

Admitted to the primary SVG parity matrix. All 13 normalized fixtures have complete Mermaid
`@11.16.1` baselines under `fixtures/upstream-svgs/cynefin/`, with per-file input/SVG hashes and
explicit provenance attestations. `compare-cynefin-svgs --check-dom` and the ordinary
`compare-all-svgs` structural DOM gate cover the committed corpus.

## Known Residuals

- Browser `getBBox()` item badge widths are represented through the headless text measurer.
- Primary matrix admission should classify this as a bounded text-metric residual instead of adding
  broad SVG normalization.

## Verification

```text
cargo run -p xtask -- compare-cynefin-svgs --check-dom --dom-mode parity --dom-decimals 3
cargo run -p xtask -- check-upstream-svgs --diagram cynefin --check-dom --dom-mode parity --dom-decimals 3
```
