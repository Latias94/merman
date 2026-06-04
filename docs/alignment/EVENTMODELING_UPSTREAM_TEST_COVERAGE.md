# EventModeling Upstream Test Coverage (Mermaid@11.15.0)

This page records the current eventmodeling fixture coverage imported into merman.

## Local Coverage

- `fixtures/eventmodeling/upstream_docs_eventmodeling_minimum.mmd`
  - detection for `eventmodeling`
  - timeframe and resetframe parsing
  - qualified entity identifiers and namespace swimlanes
  - explicit `->>` relation
  - inferred cross-swimlane relation
  - inline data and `data` block reference
  - semantic golden and layout golden

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
- Cypress image snapshot corpus from `repo-ref/mermaid/cypress/integration/rendering/eventmodeling/eventmodeling.spec.ts`.
- SVG DOM parity fixtures under fixtures/upstream-svgs/eventmodeling.
- `entity`, `note`, and `gwt` statement rendering.
