# TreeView Upstream Test Coverage (Mermaid@11.15.0)

Scope: locked Mermaid commit `41646dfd43ac83f001b03c70605feb036afae46d`.

Phase 2 admission backlog: `docs/alignment/PHASE2_PARITY_BACKLOG.md`.

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
- Cypress rendering fixtures from `repo-ref/mermaid/cypress/integration/rendering/treeView/treeView.spec.ts`:
  - `fixtures/treeView/upstream_cypress_treeview_spec_should_render_a_simple_treeview_diagram_001.mmd`
  - `fixtures/treeView/upstream_cypress_treeview_spec_should_render_a_complex_treeview_diagram_002.mmd`
  - `fixtures/treeView/upstream_cypress_treeview_spec_should_render_a_complex_treeview_diagram_with_multiple_roots_003.mmd`
  - `fixtures/treeView/upstream_cypress_treeview_spec_should_render_a_treeview_diagram_with_custom_config_004.mmd`

## Upstream SVG Baselines

- `fixtures/upstream-svgs/treeView/upstream_docs_treeview_basic.svg`
- `fixtures/upstream-svgs/treeView/upstream_cypress_treeview_spec_should_render_a_simple_treeview_diagram_001.svg`
- `fixtures/upstream-svgs/treeView/upstream_cypress_treeview_spec_should_render_a_complex_treeview_diagram_002.svg`
- `fixtures/upstream-svgs/treeView/upstream_cypress_treeview_spec_should_render_a_complex_treeview_diagram_with_multiple_roots_003.svg`
- `fixtures/upstream-svgs/treeView/upstream_cypress_treeview_spec_should_render_a_treeview_diagram_with_custom_config_004.svg`

## Compare Coverage

- Family-local command: `cargo run -p xtask -- compare-tree-view-svgs`
- Upstream baseline reproducibility: `cargo run -p xtask -- check-upstream-svgs --diagram treeView --check-dom --dom-mode parity --dom-decimals 3`
- Current DOM gate: `compare-tree-view-svgs --check-dom --dom-mode parity --dom-decimals 3`
  passes for the committed baseline corpus.

## Not Yet Covered

- Exact Langium diagnostics and offsets.
- Full strict DOM parity for the current Cypress image snapshot corpus.
