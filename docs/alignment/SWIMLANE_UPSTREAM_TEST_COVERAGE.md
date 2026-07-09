# Swimlane Upstream Test Coverage (Mermaid@11.16.0)

Scope: Mermaid tag `@11.16.0`.

## Upstream Sources

- Detector and diagram adapter:
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/swimlanes/detector.ts`
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/swimlanes/swimlanesDiagram.ts`
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/swimlanes/swimlanesDiagram.spec.ts`
- Flowchart adapter/parser reuse:
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/flowchart/flowDiagram.spec.ts`
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/flowchart/parser/flow.spec.js`
- Layout backend:
  - `repo-ref/mermaid/packages/mermaid/src/rendering-util/layout-algorithms/swimlanes/index.ts`
  - `repo-ref/mermaid/packages/mermaid/src/rendering-util/layout-algorithms/swimlanes/helpers.ts`
  - `repo-ref/mermaid/packages/mermaid/src/rendering-util/layout-algorithms/swimlanes/layoutCore.ts`
  - `repo-ref/mermaid/packages/mermaid/src/rendering-util/layout-algorithms/swimlanes/adjustLayout.ts`
  - `repo-ref/mermaid/packages/mermaid/src/rendering-util/layout-algorithms/swimlanes/orthogonalRouter/`
  - `repo-ref/mermaid/packages/mermaid/src/rendering-util/layout-algorithms/swimlanes/__tests__/`
  - `repo-ref/mermaid/packages/mermaid/src/rendering-util/layout-algorithms/swimlanes/*.ddlt.spec.ts`
- Cluster rendering and styles:
  - `repo-ref/mermaid/packages/mermaid/src/rendering-util/rendering-elements/clusters/swimlane.js`
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/swimlanes/styles.ts`
- Syntax docs: `repo-ref/mermaid/packages/mermaid/src/docs/syntax/swimlanes.md`

## Covered Locally

- Header detection and diagram id are covered by detection and registry tests.
- Flowchart parser/model reuse is covered by `parse_swimlane_reuses_flowchart_semantics_and_editor_facts`.
- Layout default precedence is covered by `parse_swimlane_layout_default_respects_user_config_precedence`.
- The deliberate lack of typed render parser is covered by
  `parse_swimlane_render_model_stays_unadmitted_until_layout_exists`.
- Config defaults are covered by `generated_default_config_contains_11_16_diagram_sections`.

## Fixture Coverage

- `fixtures/swimlane/basic_flowchart_reuse.mmd`
  - Semantic snapshot: `fixtures/swimlane/basic_flowchart_reuse.golden.json`

## Upstream SVG Baselines

Not admitted yet. The next admission step is to port the source-backed swimlane layout backend, then
generate Mermaid `@11.16.0` SVG baselines and add a family-local compare command.

## Known Residuals

- No local implementation of the upstream swimlane layout backend yet.
- No swimlane-specific edge-label node transformation, lane-aware layering, orthogonal routing, line
  hops, or automatic lane ordering yet.
- Local rendering must not fall back to ordinary Flowchart/Dagre output for `swimlane-beta`, because
  that would hide missing swimlane layout semantics.
