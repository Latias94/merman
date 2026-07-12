# Railroad Upstream Test Coverage (Mermaid@11.16.0)

Scope: Mermaid tag `@11.16.0`.

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
- Parser internals cover escaped string decoding, ABNF string slicing, and repetition bounds in
  `crates/merman-core/src/diagrams/railroad.rs`.
- LSP/editor facts cover rules, terminals, nonterminal references, and PEG nonterminal references.
- Typed render model projection for all four variants is covered by
  `parse_railroad_variants_expose_typed_render_models`.
- Layout recursion for sequence, optional/repetition arcs, and connector paths is covered by
  `railroad_layout_handles_sequence_choice_and_repetition`.
- SVG dispatch and DOM shape for rule groups, rule names, terminals, nonterminals, specials,
  connector paths, and accessible root metadata is covered by
  `render_model_dispatch_renders_railroad_svg`.

## Fixture Coverage

- `fixtures/railroad/basic_ir.mmd`
  - Semantic snapshot: `fixtures/railroad/basic_ir.golden.json`
  - Layout snapshot: `fixtures/railroad/basic_ir.layout.golden.json`
- `fixtures/railroadEbnf/choice_optional_repetition.mmd`
  - Semantic snapshot: `fixtures/railroadEbnf/choice_optional_repetition.golden.json`
  - Layout snapshot: `fixtures/railroadEbnf/choice_optional_repetition.layout.golden.json`
- `fixtures/railroadAbnf/repetition_optional_numval.mmd`
  - Semantic snapshot: `fixtures/railroadAbnf/repetition_optional_numval.golden.json`
  - Layout snapshot: `fixtures/railroadAbnf/repetition_optional_numval.layout.golden.json`
- `fixtures/railroadPeg/prefix_suffix_any.mmd`
  - Semantic snapshot: `fixtures/railroadPeg/prefix_suffix_any.golden.json`
  - Layout snapshot: `fixtures/railroadPeg/prefix_suffix_any.layout.golden.json`

## Upstream SVG Baselines

All four Railroad variants are admitted to the primary SVG parity matrix. Each normalized fixture
has a complete Mermaid `@11.16.0` baseline under its `fixtures/upstream-svgs/railroad*/` directory,
with per-file input/SVG hashes and an explicit `adopted-existing` provenance attestation. The four
family-local compare commands and the ordinary `compare-all-svgs` structural DOM gate cover the
committed corpus; browser-derived root-height differences remain in the exact root residual lane.

## Known Residuals

- Browser `getBBox()` text dimensions are represented through the headless text measurer.
- The upstream 11.16 renderer parses style options for `compactMode` and `showMarkers` but does not
  consume them in drawing; the local compatibility renderer follows the upstream rendering behavior.
- The upstream 11.16 renderer does not draw repetition separator or maximum cardinality metadata;
  the local layout keeps those parser facts in the model but does not invent extra SVG semantics.
- ABNF repetition bounds through `u64::MAX` are preserved exactly. Larger bounds produce an exact
  overflow diagnostic instead of copying JavaScript `parseInt` rounding (or `Infinity`) into the
  public integer AST; this is an explicit parser-acceptance residual from Mermaid 11.16.
