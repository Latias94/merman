# Unsupported Mermaid Family Admission Rubric

Status: Active
Baseline: pinned Mermaid `11.15.0`
Last updated: 2026-06-04

This rubric governs when a Mermaid diagram family that exists upstream but is not yet in the
implemented `merman` matrix can be promoted into a development workstream.

The goal is not to implement every upstream directory immediately. The goal is to avoid accidental
scope expansion and to make each new family fit the headless architecture: parser and semantic model
first, deterministic layout/render adapters second, upstream fixtures as evidence, and honest
root-bounds residual tracking.

## Admission Gates

A new family can enter the implemented matrix only after a child workstream has all of these:

1. **Pinned-source authority**: the family exists in `tools/upstreams/REPOS.lock.json`'s pinned
   Mermaid commit, and detector headers are documented from that commit.
2. **Semantic source plan**: the Rust parser produces one typed semantic model; compatibility JSON is
   an adapter, not a second parser-owned master path.
3. **Headless layout plan**: the renderer can be reproduced without executing Mermaid in a browser.
   Browser-only dependencies, DOM `getBBox()` phases, and third-party layout algorithms must be
   named before implementation starts.
4. **Fixture plan**: upstream syntax docs, Mermaid diagram specs, Cypress/pkg fixtures when present,
   semantic snapshots, layout snapshots, upstream SVG baselines, and an `xtask compare-*` command are
   planned up front.
5. **Residual policy**: expected root viewport and generated-measurement residuals are classified
   with the HPD taxonomy. Do not introduce fixture-keyed constants just to claim family support.
6. **API ownership**: detection, parser registry, typed render model, layout dispatch, SVG renderer,
   compare tooling, and alignment status are owned by the family workstream.

## Priority Order

| Priority | Family | Pinned 11.15 source | Recommended next action | Rationale |
|---:|---|---|---|---|
| 1 | `treeView-beta` header / `treeView` id | `packages/mermaid/src/diagrams/treeView` | Close family-local DOM residuals before main-matrix admission. | Smallest renderer and DB surface. Parser delegates to `@mermaid-js/parser`, DB builds a simple indentation tree, renderer is recursive SVG lines/text with deterministic dimensions. Phase 2 now has upstream SVG baselines and `compare-tree-view-svgs`; DOM parity still reports wrapper class residuals. |
| 2 | `ishikawa` / `ishikawa-beta` | `packages/mermaid/src/diagrams/ishikawa` | Close root width residuals, then expand theme/config/rough policy coverage. | Simple Jison grammar and tree DB. Renderer is larger and uses SVG `getBBox()` plus optional rough output, but it is still a pure headless geometry problem that fits existing text/root-bounds seams. Phase 2 now has upstream SVG baselines and `compare-ishikawa-svgs`; rough mode remains deferred. |
| 3 | `eventmodeling` | `packages/mermaid/src/diagrams/eventmodeling` | Close root width residuals, then decide `entity`, `note`, and `gwt` policy. | Renderer is modest, but DB/state construction is large and calculates swimlanes, relations, wrapped HTML/code labels, and text dimensions. Phase 2 now has upstream SVG baselines and `compare-eventmodeling-svgs`; full admission still needs DOM parity or accepted residuals plus explicit unsupported-statement policy. |
| 4 | `venn-beta` | `packages/mermaid/src/diagrams/venn` | Defer until there is a source-backed plan for `@upsetjs/venn.js` layout parity. | Parser/DB are manageable, but upstream rendering delegates circle layout to `@upsetjs/venn.js`, then uses D3 DOM and optional rough output. Do not fake this with a local circle layout unless the algorithm is audited or ported. |
| 5 | `wardley-beta` | `packages/mermaid/src/diagrams/wardley` | Defer behind smaller families unless Wardley maps become a direct requirement. | Parser and builder are feature-rich, renderer is roughly 1k lines, and the family carries many map-specific concepts: axes, stages, pipelines, links, trends, annotations, notes, accelerators, deaccelerators, and sourcing strategy overlays. Feasible headlessly, but large. |
| N/A | `railroad-*` | absent from pinned Mermaid `11.15.0` source | Do not include in the 11.15 parity backlog. Reclassify only after a baseline bump includes it. | The current pinned commit has no `packages/mermaid/src/diagrams/railroad` directory. |
| N/A | `cynefin-beta` | absent from pinned Mermaid `11.15.0` source | Do not include in the 11.15 parity backlog. Reclassify only after a baseline bump includes it. | The current pinned commit has no `packages/mermaid/src/diagrams/cynefin` directory. |

## Source Evidence

Checked against locked Mermaid commit `41646dfd43ac83f001b03c70605feb036afae46d`:

- registered lazy detector ids include `eventmodeling`, `treeView`, `ishikawa`, `venn`, and
  `wardley`; `railroad` and `cynefin` are not present in the pinned
  `packages/mermaid/src/diagrams` tree.
- `treeView` is the upstream diagram id and config key; the pinned detector accepts the
  `treeView-beta` header.
- diagram-local test sources:
  - `eventmodeling/eventmodeling.spec.ts`
  - `ishikawa/ishikawa.spec.ts`
  - `venn/parser/venn.spec.ts`
  - `venn/vennRenderer.spec.ts`
  - `wardley/wardleyBuilder.spec.ts`
  - `wardley/wardleyParser.spec.ts`
- syntax docs exist for the five pinned unsupported families:
  - `docs/syntax/eventmodeling.md`
  - `docs/syntax/ishikawa.md`
  - `docs/syntax/treeView.md`
  - `docs/syntax/venn.md`
  - `docs/syntax/wardley.md`

Approximate pinned source sizes used for priority shaping:

| Family | Parser | DB / builder | Renderer | Notes |
|---|---:|---:|---:|---|
| `treeView` | `16` lines | `69` lines | `114` lines | Lowest implementation surface; measurement still matters because labels use SVG bbox. |
| `ishikawa` | `45` lines | `79` lines | `468` lines | Pure tree geometry plus rough/getBBox phases. |
| `eventmodeling` | `25` lines | `602` lines | `138` lines | Semantic/state construction dominates. |
| `venn` | `110` lines | `116` lines | `336` lines | External Venn layout algorithm dominates risk. |
| `wardley` | `218` lines | `138` DB + `WardleyBuilder` | `971` lines | Full feature lane, not a starter task. |

These counts are queue-shaping hints, not completion estimates.

## Workstream Template For A New Family

The first implementation slice for any admitted family should be:

- detector + typed parser + semantic snapshots,
- source-test ports for parser/DB behavior,
- layout model with a small deterministic fixture set,
- Stage B SVG renderer for minimal examples,
- upstream SVG baseline generation using the official Mermaid CLI,
- dedicated `xtask compare-<family>-svgs`,
- alignment status update that marks unsupported residuals honestly.

Do not add the family to the main coverage matrix until parse, layout, render, upstream SVG baseline,
and compare tooling all exist for at least a minimal source-backed fixture set.
