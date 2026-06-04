# Ishikawa Upstream Test Coverage (Mermaid@11.15.0)

Scope: locked Mermaid commit `41646dfd43ac83f001b03c70605feb036afae46d`.

## Upstream Sources

- Parser tests: `repo-ref/mermaid/packages/mermaid/src/diagrams/ishikawa/ishikawa.spec.ts`
- Rendering tests: `repo-ref/mermaid/cypress/integration/rendering/ishikawa/ishikawa.spec.ts`
- Syntax docs: `repo-ref/mermaid/docs/syntax/ishikawa.md`

## Covered Locally

- `should parse a basic Ishikawa hierarchy`:
  - parser unit coverage in `crates/merman-core/src/diagrams/ishikawa.rs`
- `should support an unindented root with nested causes`:
  - covered by the same indentation/base-level parser path
- `should handle effect indented more than causes`:
  - parser unit coverage in `crates/merman-core/src/diagrams/ishikawa.rs`
- Basic typed SVG output:
  - `crates/merman-render/tests/ishikawa_svg_test.rs`

## Fixture Coverage

- `fixtures/ishikawa/upstream_docs_ishikawa_basic.mmd`
  - source: `repo-ref/mermaid/docs/syntax/ishikawa.md`
  - semantic snapshot: `fixtures/ishikawa/upstream_docs_ishikawa_basic.golden.json`
  - layout snapshot: `fixtures/ishikawa/upstream_docs_ishikawa_basic.layout.golden.json`

## Not Yet Covered

- Upstream SVG baselines under fixtures/upstream-svgs/ishikawa.
- Dedicated `xtask compare-ishikawa-svgs`.
- Hand-drawn / rough.js renderer branch.
- Full Cypress image snapshot corpus import.
