# TreeView Upstream Test Coverage (Mermaid@11.15.0)

Scope: locked Mermaid commit `41646dfd43ac83f001b03c70605feb036afae46d`.

## Upstream Sources

- Parser tests: `repo-ref/mermaid/packages/parser/tests/treeView.test.ts`
- Rendering tests: `repo-ref/mermaid/cypress/integration/rendering/treeView/treeView.spec.ts`
- Syntax docs: `repo-ref/mermaid/docs/syntax/treeView.md`

## Covered Locally

- `should parse empty treeView`:
  - parser path covered by `crates/merman-core/src/diagrams/tree_view.rs`
- `should parse a treeView with only a root node`:
  - parser unit coverage in `crates/merman-core/src/diagrams/tree_view.rs`
- `should parse a treeView with multiple words within a node`:
  - quoted-string parser supports spaces in node names
- `should parse a treeView with child nodes`:
  - parser unit coverage in `crates/merman-core/src/diagrams/tree_view.rs`
- `should parse a treeView with title`
- `should parse a treeView with accTitle`
- `should parse a treeView with accDescr`
- `should parse a treeView with multiple accessibility attributes`
  - parser unit coverage in `crates/merman-core/src/diagrams/tree_view.rs`
- Cypress custom config example:
  - `crates/merman-render/tests/tree_view_svg_test.rs`

## Fixture Coverage

- `fixtures/treeView/upstream_docs_treeview_basic.mmd`
  - source: `repo-ref/mermaid/docs/syntax/treeView.md`
  - semantic snapshot: `fixtures/treeView/upstream_docs_treeview_basic.golden.json`
  - layout snapshot: `fixtures/treeView/upstream_docs_treeview_basic.layout.golden.json`

## Not Yet Covered

- Upstream SVG baselines under fixtures/upstream-svgs/treeView.
- Dedicated `xtask compare-tree-view-svgs`.
- Full Cypress image snapshot corpus import.
