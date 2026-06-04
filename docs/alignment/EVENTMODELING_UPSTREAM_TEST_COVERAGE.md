# EventModeling Upstream Test Coverage (Mermaid@11.15.0)

This page records the current eventmodeling fixture coverage imported into merman.

Phase 2 admission backlog: `docs/alignment/PHASE2_PARITY_BACKLOG.md`.

## Local Coverage

- `fixtures/eventmodeling/upstream_docs_eventmodeling_minimum.mmd`
  - detection for `eventmodeling`
  - timeframe and resetframe parsing
  - qualified entity identifiers and namespace swimlanes
  - explicit `->>` relation
  - inferred cross-swimlane relation
  - inline data and `data` block reference
  - semantic golden and layout golden
- Cypress rendering fixtures from `repo-ref/mermaid/cypress/integration/rendering/eventmodeling/eventmodeling.spec.ts`:
  - `fixtures/eventmodeling/upstream_cypress_eventmodeling_spec_renders_a_state_view_pattern_001.mmd`
  - `fixtures/eventmodeling/upstream_cypress_eventmodeling_spec_renders_a_state_change_pattern_002.mmd`
  - `fixtures/eventmodeling/upstream_cypress_eventmodeling_spec_renders_a_translation_pattern_003.mmd`
  - `fixtures/eventmodeling/upstream_cypress_eventmodeling_spec_renders_with_data_block_reference_004.mmd`
  - `fixtures/eventmodeling/upstream_cypress_eventmodeling_spec_renders_with_qualified_names_005.mmd`
  - `fixtures/eventmodeling/upstream_cypress_eventmodeling_spec_renders_with_multiple_source_relations_006.mmd`

## Upstream SVG Baselines

- `fixtures/upstream-svgs/eventmodeling/upstream_docs_eventmodeling_minimum.svg`
- `fixtures/upstream-svgs/eventmodeling/upstream_cypress_eventmodeling_spec_renders_a_state_view_pattern_001.svg`
- `fixtures/upstream-svgs/eventmodeling/upstream_cypress_eventmodeling_spec_renders_a_state_change_pattern_002.svg`
- `fixtures/upstream-svgs/eventmodeling/upstream_cypress_eventmodeling_spec_renders_a_translation_pattern_003.svg`
- `fixtures/upstream-svgs/eventmodeling/upstream_cypress_eventmodeling_spec_renders_with_data_block_reference_004.svg`
- `fixtures/upstream-svgs/eventmodeling/upstream_cypress_eventmodeling_spec_renders_with_qualified_names_005.svg`
- `fixtures/upstream-svgs/eventmodeling/upstream_cypress_eventmodeling_spec_renders_with_multiple_source_relations_006.svg`

## Compare Coverage

- Family-local command: `cargo run -p xtask -- compare-eventmodeling-svgs`
- Upstream baseline reproducibility: `cargo run -p xtask -- check-upstream-svgs --diagram eventmodeling --check-dom --dom-mode parity --dom-decimals 3`
- Current DOM gate: `compare-eventmodeling-svgs --check-dom --dom-mode parity --dom-decimals 3`
  passes for the committed baseline corpus.

## Upstream Sources Reviewed

- `repo-ref/mermaid/packages/parser/src/language/eventmodeling/event-modeling.langium`
- `repo-ref/mermaid/packages/parser/tests/eventmodeling.test.ts`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/eventmodeling/db.ts`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/eventmodeling/renderer.ts`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/eventmodeling/eventmodeling.spec.ts`
- `repo-ref/mermaid/cypress/integration/rendering/eventmodeling/eventmodeling.spec.ts`
- `repo-ref/mermaid/docs/syntax/eventmodeling.md`

## Deferred Coverage

- Full upstream parser fixtures from `repo-ref/mermaid/packages/parser/tests/eventmodeling.test.ts`.
- `entity`, `note`, and `gwt` statement rendering.
- Full strict DOM parity for the current Cypress image snapshot corpus.
