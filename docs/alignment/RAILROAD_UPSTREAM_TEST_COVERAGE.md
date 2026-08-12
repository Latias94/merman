# Railroad Upstream Test Coverage (Mermaid@11.16.1)

Scope: Mermaid tag `@11.16.1`.

## Upstream Sources

- Detectors:
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/railroad/railroadDetector.spec.ts`
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/railroad/ebnfDetector.spec.ts`
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/railroad/abnfDetector.spec.ts`
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/railroad/pegDetector.spec.ts`
- Parsers:
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/railroad/parser/railroadDiagram.spec.ts`
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/railroad/parser/ebnfDiagram.spec.ts`
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/railroad/parser/abnfDiagram.spec.ts`
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/railroad/parser/pegDiagram.spec.ts`
- DB/model and renderer:
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/railroad/railroadDb.spec.ts`
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/railroad/railroadRenderer.spec.ts`
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/railroad/styles.spec.ts`
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/railroad/railroadRenderer.ts`
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/railroad/styles.ts`
- Syntax docs: `repo-ref/mermaid/packages/mermaid/src/docs/syntax/railroad.md`

## Covered Locally

- Header detection and diagram ids for `railroad`, `railroadEbnf`, `railroadAbnf`, and `railroadPeg`
  are covered by registry/detection tests.
- Parser coverage for IR, EBNF, ABNF, and PEG variants lives in
  `crates/merman-core/src/tests/railroad.rs`.
- Parser internals cover escaped string decoding, ABNF string slicing, and JavaScript-compatible
  binary64 repetition bounds, including rounding, unbounded maxima, and positive infinity, in
  `crates/merman-core/src/diagrams/railroad.rs`.
- LSP/editor facts cover rules, terminals, nonterminal references, and PEG nonterminal references.
- Typed render model projection for all four variants is covered by
  `parse_railroad_variants_expose_typed_render_models`.
- Layout recursion for sequence, optional/repetition arcs, and connector paths is covered by
  `railroad_layout_handles_sequence_choice_and_repetition`; zero, finite nonzero, and infinite
  repetition minima are covered by `railroad_repetition_bypass_depends_only_on_zero_minimum`.
- CLI `parse --meta` coverage verifies rounded finite Railroad bounds by parsed binary64 value and
  infinity as JSON `null`; exact large-number JSON spelling is not a compatibility requirement.
- SVG dispatch and DOM shape for rule groups, rule names, terminals, nonterminals, specials,
  connector paths, and accessible root metadata is covered by
  `render_model_dispatch_renders_railroad_svg`.

## Fixture Coverage

- The active corpus contains 13 fixtures, partitioned by source:
  - 9 Mermaid Cypress render cases from `repo-ref/mermaid/cypress/integration/rendering/railroad/railroad.spec.ts`:
    five IR cases, two EBNF cases, one ABNF case, and one PEG case.
  - 4 local syntax/parser fixtures, one for each dialect, covering the compact IR, EBNF, ABNF, and
    PEG forms used by the semantic/editor tests:
    `fixtures/railroad/basic_ir.mmd`, `fixtures/railroadEbnf/choice_optional_repetition.mmd`,
    `fixtures/railroadAbnf/repetition_optional_numval.mmd`, and
    `fixtures/railroadPeg/prefix_suffix_any.mmd`.
- Every active fixture has a semantic golden, a typed-layout golden, and a pinned Mermaid SVG. The
  Cypress set supplies source-backed renderer evidence; the four local fixtures keep parser/editor
  and dialect-specific edge cases in the ordinary snapshot lane.

## Upstream SVG Baselines

All four Railroad variants are admitted to the primary SVG parity matrix. The 13 normalized fixtures
have complete Mermaid `@11.16.1` baselines under the four `fixtures/upstream-svgs/railroad*/`
directories, with per-file input/SVG hashes and explicit provenance attestations. The four
family-local compare commands and the ordinary `compare-all-svgs` structural DOM gate cover the
committed corpus; browser-derived root-height differences remain visible in the browser root
diagnostic artifact.

## Known Residuals

- Browser `getBBox()` text dimensions are represented through the headless text measurer.
- The upstream 11.16 renderer parses style options for `compactMode` and `showMarkers` but does not
  consume them in drawing; the local compatibility renderer follows the upstream rendering behavior.
- The upstream 11.16 renderer does not draw repetition separator or maximum cardinality metadata;
  the local layout keeps those parser facts in the model but does not invent extra SVG semantics.

## Verification

```text
cargo run -p xtask -- compare-railroad-svgs --check-dom --dom-mode parity --dom-decimals 3
cargo run -p xtask -- compare-railroad-ebnf-svgs --check-dom --dom-mode parity --dom-decimals 3
cargo run -p xtask -- compare-railroad-abnf-svgs --check-dom --dom-mode parity --dom-decimals 3
cargo run -p xtask -- compare-railroad-peg-svgs --check-dom --dom-mode parity --dom-decimals 3
cargo run -p xtask -- check-upstream-svgs --diagram railroad --check-dom --dom-mode parity --dom-decimals 3
```
