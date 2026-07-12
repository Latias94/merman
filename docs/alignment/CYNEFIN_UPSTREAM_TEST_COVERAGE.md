# Cynefin Upstream Test Coverage (Mermaid@11.16.0)

Scope: Mermaid tag `@11.16.0`.

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

- `fixtures/cynefin/basic_domains_transitions.mmd`
  - Semantic snapshot: `fixtures/cynefin/basic_domains_transitions.golden.json`
  - Layout snapshot: `fixtures/cynefin/basic_domains_transitions.layout.golden.json`

## Upstream SVG Baselines

Admitted to the primary SVG parity matrix. The normalized fixture has a complete Mermaid
`@11.16.0` baseline under `fixtures/upstream-svgs/cynefin/`, with per-file input/SVG hashes and an
explicit `adopted-existing` provenance attestation. `compare-cynefin-svgs --check-dom` and the
ordinary `compare-all-svgs` structural DOM gate cover the committed corpus.

## Known Residuals

- Browser `getBBox()` item badge widths are represented through the headless text measurer.
- Primary matrix admission should classify this as a bounded text-metric residual instead of adding
  broad SVG normalization.
