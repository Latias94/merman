# Mermaid 11.15 Baseline Upgrade - Handoff

Status: Active
Last updated: 2026-05-31

## Current State

The workstream is open. M15-020 is complete: sequence `autonumber` now accepts Mermaid 11.15
hundredths-place decimal start and step values, rejects thousandths, serializes integer values
without unnecessary `.0`, and renders decimal sequence numbers with two-decimal accumulation.
M15-030 is complete: flowchart shape-data accepts `datastore` and `data-store`, sizes them like
Mermaid's datastore `drawRect` path, and renders a top/bottom-border rect via
`stroke-dasharray=width height` instead of using the existing `stored-data` / `bow-rect` geometry.
M15-031 is complete: the generated default config now uses `flowchart.curve=rounded`, SVG edge
rendering supports Mermaid's rounded quadratic-corner path, and explicit `curve: basis` still
renders smooth cubic paths.
M15-040 is complete: Architecture now carries Mermaid 11.15 FCoSE defaults and wires
`randomize`, `nodeSeparation`, `idealEdgeLengthMultiplier`, `edgeElasticity`, `numIter`, and
deterministic `seed` through the local indexed FCoSE path. Default output remains deterministic;
configured randomization and layout-budget changes are covered by layout tests.
M15-050 is complete: Sankey now exposes Mermaid 11.15 defaults for `nodeWidth`, `nodePadding`,
`labelStyle`, and `nodeColors`; layout reads configured width/padding, SVG rendering applies custom
node colors to nodes and links, and `labelStyle=outlined` emits Mermaid-style background/foreground
labels. Sankey layout goldens were refreshed for the upstream default padding change.
M15-060 is complete: xyChart now exposes Mermaid 11.15 `showDataLabelOutsideBar=false`, bar data
labels use configured `themeVariables.xyChart.dataLabelColor` with `primaryTextColor` fallback, and
vertical/horizontal outside-bar placement is covered by public SVG tests. No layout snapshot change
was needed because the behavior is render-layer only.
M15-070 is complete: class diagrams now default to Mermaid 11.15 hierarchical namespaces for dotted
and nested namespace syntax, namespace notes are parented to the active namespace, and
`class.hierarchicalNamespaces=false` compacts the semantic model back to the flat dotted namespace
behavior used by <=11.14. Class semantic and layout goldens were refreshed only under
`fixtures/class`; non-class snapshot churn was intentionally not kept.
M15-080 is complete: c4, journey, timeline, and sequence marker IDs now use Mermaid 11.14-style
`<diagram-id>-<local-id>` scoping, marker references point at the scoped IDs, sequence actor-man
control markers follow the same helper, and sequence CSS now uses `[id$="..."]` selectors for
prefixed marker IDs.

## Active Task

- Task ID: M15-090
- Owner: unassigned
- Files: `docs/workstreams/mermaid-11-15-baseline-upgrade`, `docs/alignment`
- Validation: `DESIGN.md`, `STATUS.md`, or follow-on workstreams record support/defer/out-of-scope status
- Status: READY
- Review: Required before task acceptance
- Evidence: To be recorded in `EVIDENCE_AND_GATES.md`

## Decisions Since Last Update

- Do not update `README.md`, ADR-0001, or `REPOS.lock.json` to `11.15.0` until implemented scope and evidence agree.
- Start with decimal sequence `autonumber` because it is a bounded semantic compatibility slice.
- Sequence decimal `autonumber` is done and has fresh core/render/fmt evidence.
- `datastore` is a new rectangular shape in Mermaid 11.15 and must not be mapped to
  `stored-data` / `bow-rect`.
- The default flowchart curve baseline change was small in this repo: package gates passed without
  broad fixture churn.
- Architecture FCoSE remains deterministic by default (`randomize=false`, `seed=1`), but manatee's
  generic FCoSE API keeps cytoscape-fcose's library default of `randomize=true`.
- Sankey follows the 11.15 default padding baseline (`nodePadding=12`, plus 15 when values are
  shown), so existing Sankey layout goldens changed intentionally.
- Sankey `nodeColors` is represented as `{}` in local generated JSON because upstream TypeScript
  exposes the key as `undefined`; render behavior is equivalent for the default case.
- xyChart data-label outside placement is a render-layer change in this repo; layout goldens should
  remain stable unless later work moves label extents into layout.
- xyChart data-label color preserves the old effective black fallback unless
  `themeVariables.xyChart.dataLabelColor` or `themeVariables.primaryTextColor` is configured.
- Class namespace baseline is hierarchical by default in the semantic model and layout renderer.
- `class.hierarchicalNamespaces=false` is applied before layout/SVG rendering so fixture and render
  paths share one compacted model.
- Class layout/semantic golden changes are intentionally limited to `fixtures/class`.
- Internal marker IDs in newly touched SVG renderers should be derived with the shared
  `scoped_svg_id` / `scoped_svg_url` helpers so exact-ID regressions are easier to spot.
- Sequence marker CSS must stay suffix-compatible because marker IDs are no longer stable exact
  strings once a caller supplies `SvgRenderOptions.diagram_id`.

## Blockers

- None known for M15-090. This is a scope and baseline metadata decision task, not a renderer
  implementation task.

## Next Recommended Action

- Execute M15-090. Revisit the 11.13-11.15 release delta and document support, defer, or
  out-of-scope status for new diagram families before bumping baseline metadata in M15-100.
