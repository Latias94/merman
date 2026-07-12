# Ishikawa Upstream Test Coverage (Mermaid@11.16.0)

Scope: locked Mermaid commit `7c0cafcf42e76bfaf79d0cbbd12edb986612f014`.

Phase 2 admission backlog: `docs/alignment/PHASE2_PARITY_BACKLOG.md`.

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
  - asserts typed pair, upper/lower branch, cause-label group, and sub-group ownership

## Fixture Coverage

- `fixtures/ishikawa/upstream_docs_ishikawa_basic.mmd`
  - source: `repo-ref/mermaid/docs/syntax/ishikawa.md`
  - semantic snapshot: `fixtures/ishikawa/upstream_docs_ishikawa_basic.golden.json`
  - layout snapshot: `fixtures/ishikawa/upstream_docs_ishikawa_basic.layout.golden.json`
- Cypress rendering fixtures from `repo-ref/mermaid/cypress/integration/rendering/ishikawa/ishikawa.spec.ts`:
  - `fixtures/ishikawa/upstream_cypress_ishikawa_spec_1_should_render_a_simple_ishikawa_diagram_001.mmd`
  - `fixtures/ishikawa/upstream_cypress_ishikawa_spec_2_should_render_with_many_causes_on_both_sides_002.mmd`
  - `fixtures/ishikawa/upstream_cypress_ishikawa_spec_3_should_render_with_deeply_nested_causes_003.mmd`
  - `fixtures/ishikawa/upstream_cypress_ishikawa_spec_4_should_render_with_a_single_cause_004.mmd`
  - `fixtures/ishikawa/upstream_cypress_ishikawa_spec_5_should_render_with_no_children_root_only_005.mmd`
  - `fixtures/ishikawa/upstream_cypress_ishikawa_spec_7_should_render_with_forest_theme_007.mmd`
  - `fixtures/ishikawa/upstream_cypress_ishikawa_spec_8_should_render_with_dark_theme_008.mmd`
  - `fixtures/ishikawa/upstream_cypress_ishikawa_spec_9_should_render_with_custom_diagrampadding_009.mmd`
  - `fixtures/ishikawa/upstream_cypress_ishikawa_spec_10_should_render_when_usemaxwidth_is_true_010.mmd`
  - `fixtures/ishikawa/upstream_cypress_ishikawa_spec_11_should_render_when_usemaxwidth_is_false_011.mmd`
  - `fixtures/ishikawa/upstream_cypress_ishikawa_spec_12_should_render_correctly_when_effect_is_indented_more_than_cau_010.mmd`

## Upstream SVG Baselines

- `fixtures/upstream-svgs/ishikawa/upstream_docs_ishikawa_basic.svg`
- `fixtures/upstream-svgs/ishikawa/upstream_cypress_ishikawa_spec_1_should_render_a_simple_ishikawa_diagram_001.svg`
- `fixtures/upstream-svgs/ishikawa/upstream_cypress_ishikawa_spec_2_should_render_with_many_causes_on_both_sides_002.svg`
- `fixtures/upstream-svgs/ishikawa/upstream_cypress_ishikawa_spec_3_should_render_with_deeply_nested_causes_003.svg`
- `fixtures/upstream-svgs/ishikawa/upstream_cypress_ishikawa_spec_4_should_render_with_a_single_cause_004.svg`
- `fixtures/upstream-svgs/ishikawa/upstream_cypress_ishikawa_spec_5_should_render_with_no_children_root_only_005.svg`
- `fixtures/upstream-svgs/ishikawa/upstream_cypress_ishikawa_spec_7_should_render_with_forest_theme_007.svg`
- `fixtures/upstream-svgs/ishikawa/upstream_cypress_ishikawa_spec_8_should_render_with_dark_theme_008.svg`
- `fixtures/upstream-svgs/ishikawa/upstream_cypress_ishikawa_spec_9_should_render_with_custom_diagrampadding_009.svg`
- `fixtures/upstream-svgs/ishikawa/upstream_cypress_ishikawa_spec_10_should_render_when_usemaxwidth_is_true_010.svg`
- `fixtures/upstream-svgs/ishikawa/upstream_cypress_ishikawa_spec_11_should_render_when_usemaxwidth_is_false_011.svg`
- `fixtures/upstream-svgs/ishikawa/upstream_cypress_ishikawa_spec_12_should_render_correctly_when_effect_is_indented_more_than_cau_010.svg`

## Compare Coverage

- Family-local command: `cargo run -p xtask -- compare-ishikawa-svgs`
- Upstream baseline reproducibility: `cargo run -p xtask -- check-upstream-svgs --diagram ishikawa --check-dom --dom-mode parity --dom-decimals 3`
- Current DOM gates pass all 12 committed baselines:
  - `compare-ishikawa-svgs --check-dom --dom-mode structure --dom-decimals 3`
  - `compare-ishikawa-svgs --check-dom --dom-mode parity --dom-decimals 3`
- Structure convergence comes from the typed layout/render tree matching Mermaid 11.16's
  `ishikawa-pair`, `ishikawa-label-group`, and `ishikawa-sub-group` ownership. It does not use
  comparator normalization or fixture-specific exceptions.

## Not Yet Covered

- Hand-drawn / rough.js renderer branch.
- Very deep nested Cypress image snapshot case.
- Full strict DOM parity for the current Cypress image snapshot corpus.
