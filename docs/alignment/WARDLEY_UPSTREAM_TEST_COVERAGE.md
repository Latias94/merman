# Wardley Upstream Test Coverage (Mermaid@11.16.0)

Scope: pinned Mermaid `11.16.0`, commit
`7c0cafcf42e76bfaf79d0cbbd12edb986612f014`.

## Pinned Sources

- Cypress render corpus:
  `repo-ref/mermaid/cypress/integration/rendering/wardley/wardley.spec.js`
- Detector: `repo-ref/mermaid/packages/mermaid/src/diagrams/wardley/wardleyDetector.ts`
- Parser adapter: `repo-ref/mermaid/packages/mermaid/src/diagrams/wardley/wardleyParser.ts`
- Builder: `repo-ref/mermaid/packages/mermaid/src/diagrams/wardley/wardleyBuilder.ts`
- Database: `repo-ref/mermaid/packages/mermaid/src/diagrams/wardley/wardleyDb.ts`
- Renderer: `repo-ref/mermaid/packages/mermaid/src/diagrams/wardley/wardleyRenderer.ts`
- Styles: `repo-ref/mermaid/packages/mermaid/src/diagrams/wardley/styles.ts`

## Source-Backed Fixture Corpus

All ten Cypress render cases are committed. Diagram bodies preserve the upstream templates; the
four parameterized theme options are represented by equivalent Mermaid frontmatter.

| # | Fixture | Upstream behavior |
| ---: | --- | --- |
| 1 | `fixtures/wardley/upstream_cypress_wardley_spec_1_should_render_tea_shop_001.mmd` | Anchors, components, links, evolution, and notes |
| 2 | `fixtures/wardley/upstream_cypress_wardley_spec_2_should_render_data_evolution_stages_002.mmd` | Custom evolution stages |
| 3 | `fixtures/wardley/upstream_cypress_wardley_spec_3_should_render_pipelines_003.mmd` | Pipeline membership and evolution links |
| 4 | `fixtures/wardley/upstream_cypress_wardley_spec_4_should_render_link_types_and_annotations_004.mmd` | Link operators, labels, and annotations |
| 5 | `fixtures/wardley/upstream_cypress_wardley_spec_5_should_render_custom_canvas_size_005.mmd` | Custom canvas size |
| 6 | `fixtures/wardley/upstream_cypress_wardley_spec_should_render_under_the_dark_theme_006.mmd` | Dark theme roles |
| 7 | `fixtures/wardley/upstream_cypress_wardley_spec_should_render_under_the_forest_theme_007.mmd` | Forest theme roles |
| 8 | `fixtures/wardley/upstream_cypress_wardley_spec_should_render_under_the_neutral_theme_008.mmd` | Neutral theme roles |
| 9 | `fixtures/wardley/upstream_cypress_wardley_spec_should_render_under_the_base_theme_009.mmd` | Base theme roles |
| 10 | `fixtures/wardley/upstream_cypress_wardley_spec_6_should_render_gpt_tokeniser_architecture_010.mmd` | Large map and dense label/link coverage |

## Committed Evidence

Each fixture has all four admission artifacts:

- semantic golden: `fixtures/wardley/*.golden.json`;
- typed-layout golden: `fixtures/wardley/*.layout.golden.json`;
- pinned Mermaid SVG: `fixtures/upstream-svgs/wardley/*.svg`;
- schema-v2 generated-complete provenance with input/output hashes:
  `fixtures/upstream-svgs/wardley/_baseline-manifest.json`.

The manifest has ten fixture entries, `complete: true`, generated attestation, and an empty
`excluded` object.

## Compare Coverage

| Gate | Result | Accepted residuals |
| --- | ---: | ---: |
| `cargo run -p xtask -- compare-wardley-svgs --check-dom --dom-mode parity` | 10/10 | 0 |
| `cargo run -p xtask -- compare-wardley-svgs --check-dom --dom-mode parity-root` | 10/10 | 0 |

No fixture uses a structure-only downgrade, exclusion, fixture rewrite, or accepted-residual
entry. General browser font-rasterization variability remains an environmental boundary, not an
accepted failing fixture in this corpus.

## Verification

```bash
cargo run -p xtask -- update-snapshots --diagram wardley
cargo run -p xtask -- update-layout-snapshots --diagram wardley
cargo run -p xtask -- check-upstream-svgs --diagram wardley
cargo run -p xtask -- compare-wardley-svgs --check-dom --dom-mode parity
cargo run -p xtask -- compare-wardley-svgs --check-dom --dom-mode parity-root
cargo run -p xtask -- check-alignment
```
