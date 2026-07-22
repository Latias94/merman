# Swimlane Upstream Test Coverage (Mermaid@11.16.0)

Scope: Mermaid tag `@11.16.0`, commit
`7c0cafcf42e76bfaf79d0cbbd12edb986612f014`.

## Upstream Sources

- Detector and diagram adapter:
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/swimlanes/detector.ts`
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/swimlanes/swimlanesDiagram.ts`
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/swimlanes/swimlanesDiagram.spec.ts`
- Flowchart adapter/parser reuse:
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/flowchart/flowDiagram.spec.ts`
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/flowchart/parser/flow.spec.js`
- Layout and routing:
  - `repo-ref/mermaid/packages/mermaid/src/rendering-util/layout-algorithms/swimlanes/`
  - `repo-ref/mermaid/packages/mermaid/src/rendering-util/layout-algorithms/swimlanes/orthogonalRouter/`
  - `repo-ref/mermaid/packages/mermaid/src/rendering-util/layout-algorithms/swimlanes/__tests__/`
  - `repo-ref/mermaid/packages/mermaid/src/rendering-util/layout-algorithms/swimlanes/*.ddlt.spec.ts`
- Cluster rendering and styles:
  - `repo-ref/mermaid/packages/mermaid/src/rendering-util/rendering-elements/clusters/swimlane.js`
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/swimlanes/styles.ts`
- Syntax docs: `repo-ref/mermaid/docs/syntax/swimlanes.md`

## Admission State

`swimlane` is in the primary SVG matrix. It owns a typed layout and routing path rather than
silently falling back to ordinary Flowchart/Dagre output. The ordinary active corpus contains
three source-backed fixtures:

- `fixtures/swimlane/upstream_docs_basic.mmd`
- `fixtures/swimlane/upstream_7_car_sales_constr.mmd`
- `fixtures/swimlane/basic_flowchart_reuse.mmd`

Each has semantic and layout goldens plus a pinned Mermaid SVG under
`fixtures/upstream-svgs/swimlane/`. The family-local gate is
`cargo run -p xtask -- compare-swimlane-svgs`.

## DDLT/Layout Corpus

Mermaid 11.16 ships 30 DDLT/layout inputs. They are preserved verbatim under
`fixtures/swimlane/_upstream_ddlt/` and intentionally remain outside ordinary snapshot admission:
they are layout evidence, not independent semantic/render fixtures. The corpus spans:

- LR and TB directions, including border-hugging and wide-lane cases;
- nested lanes, loose nodes, lane ownership, and node placement;
- edge labels, cycles, disagreeing exits, and process-overflow regressions;
- constrained and optimized rank configuration, including the car-sales and query-process cases.

Two executable sweeps consume all 30 files:

- `crates/merman-render/tests/swimlane_layout_test.rs` checks parse recovery, finite geometry,
  non-empty lanes/nodes, valid bounds, orthogonal routes, and deterministic repeated layout;
- `crates/merman/tests/swimlane_typed_render.rs` renders every file through the typed path and
  checks valid SVG/XML, lane/node ownership, and the accessible Swimlane root.

This gives 33 source-backed inputs in total (3 active fixtures + 30 DDLT inputs) without duplicating
the same high-volume layout corpus in the semantic snapshot matrix.

## Focused Local Coverage

- Flowchart semantic reuse and editor facts are covered by
  `parse_swimlane_reuses_flowchart_semantics_and_editor_facts`.
- Default layout precedence and explicit `layout: swimlane` behavior are covered by the core
  Flowchart tests.
- Synthetic default-lane ownership and explicit reuse of `__swimlane_default__` are covered by
  `swimlane_layout_test.rs`.
- Explicit edge/link curves, edge-label waypoints, cycle reversal, all four directions, and
  deterministic output are covered by the same test module.
- The typed SVG path has dedicated root/cluster/node/edge DOM assertions and the full 30-file DDLT
  XML sweep in `crates/merman/tests/swimlane_typed_render.rs`.

## Verification

```text
cargo nextest run -p merman-render --test swimlane_layout_test
cargo nextest run -p merman --test swimlane_typed_render
cargo run -p xtask -- compare-swimlane-svgs --check-dom --dom-mode parity --dom-decimals 3
cargo run -p xtask -- check-upstream-svgs --diagram swimlane --check-dom --dom-mode parity --dom-decimals 3
```

The DDLT files are not counted as active semantic fixtures, and no comparator downgrade is inferred
from their path or name. Any future promotion must add the normal semantic, layout, SVG, provenance,
and family-compare evidence through the existing import transaction.
