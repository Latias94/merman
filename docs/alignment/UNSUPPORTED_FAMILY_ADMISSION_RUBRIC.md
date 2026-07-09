# Unsupported Mermaid Family Admission Rubric

Status: Active
Baseline: pinned Mermaid `11.16.0`
Last updated: 2026-07-09

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

| Priority | Family | Pinned 11.16 source | Recommended next action | Rationale |
|---:|---|---|---|---|
| 1 | `treeView-beta` header / `treeView` id | `packages/mermaid/src/diagrams/treeView` | Track root viewport residuals separately from the primary matrix. | Smallest renderer and DB surface. Parser delegates to `@mermaid-js/parser`, DB builds a simple indentation tree, renderer is recursive SVG lines/text with deterministic dimensions. Phase 2 now has upstream SVG baselines, `compare-tree-view-svgs`, a green family-local DOM parity gate, and primary `compare-all-svgs` admission; root viewport residuals remain browser text-metric derived. |
| 2 | `ishikawa` / `ishikawa-beta` | `packages/mermaid/src/diagrams/ishikawa` | Decide rough/handDrawn policy before importing rough-mode coverage. | Simple Jison grammar and tree DB. Renderer is larger and uses SVG `getBBox()` plus optional rough output, but it is still a pure headless geometry problem that fits existing text/root-bounds seams. Phase 2 now has upstream SVG baselines, config/theme coverage, `compare-ishikawa-svgs`, a green family-local DOM parity gate, and primary `compare-all-svgs` admission; rough mode remains deferred. |
| 3 | `eventmodeling` | `packages/mermaid/src/diagrams/eventmodeling` | Decide `entity`, `note`, and `gwt` policy before unsupported-statement rendering. | Renderer is modest, but DB/state construction is large and calculates swimlanes, relations, wrapped HTML/code labels, and text dimensions. Phase 2 now has upstream SVG baselines, parser-source coverage, `compare-eventmodeling-svgs`, a green family-local DOM parity gate, and primary `compare-all-svgs` admission; explicit unsupported-statement policy is still needed. |
| 4 | `swimlane-beta` header / `swimlane` id | `packages/mermaid/src/diagrams/swimlanes` | Port the swimlane layout/render path on top of Flowchart semantics before SVG admission. | New in the pinned 11.16 baseline. Detector/header/config/admission visibility exists locally, the parser/editor-facts path reuses Flowchart as upstream does, and semantic fixtures are admitted as parse-only evidence. Source-backed swimlane layout, SVG baselines, and compare tooling are not admitted yet. |
| 5 | `railroad-beta`, `railroad-ebnf-beta`, `railroad-abnf-beta`, `railroad-peg-beta` | `packages/mermaid/src/diagrams/railroad` | Add 11.16 upstream SVG baselines and a family compare command before primary matrix admission. | New in the pinned 11.16 baseline. The upstream family has separate detector/parser specs for base, EBNF, ABNF, and PEG variants. Detector/header/config/admission visibility, parser/editor facts, unified AST JSON, semantic fixtures, typed layout, and local SVG renderer exist locally. The remaining admission gap is upstream SVG baselines, compare tooling, and documenting headless text-measurement residuals. |
| 6 | `cynefin-beta` | `packages/mermaid/src/diagrams/cynefin` | Add 11.16 upstream SVG baselines and a family compare command before primary matrix admission. | New in the pinned 11.16 baseline. Detector/header/config/admission visibility, parser/editor facts, semantic fixtures, typed layout, and local SVG renderer exist locally. The remaining admission gap is upstream SVG baselines, compare tooling, and documenting the headless `getBBox()` text-measurement residual. |
| 7 | `wardley-beta` | `packages/mermaid/src/diagrams/wardley` | Defer behind smaller families unless Wardley maps become a direct requirement. | Parser and builder are feature-rich, renderer is roughly 1k lines, and the family carries many map-specific concepts: axes, stages, pipelines, links, trends, annotations, notes, accelerators, deaccelerators, and sourcing strategy overlays. Feasible headlessly, but large. |
| admitted | `venn-beta` | `packages/mermaid/src/diagrams/venn` | Keep expanding classic fixtures before considering RoughJS `handDrawn` parity; see `VENN_BETA_ADMISSION_PLAN.md`. | Classic SVG output now has a source-backed `@upsetjs/venn.js@2.0.0` layout kernel, semantic/layout fixtures, upstream SVG baselines, and a green family-local compare gate. |

## Source Evidence

Checked against locked Mermaid commit `7c0cafcf42e76bfaf79d0cbbd12edb986612f014`
(`mermaid@11.16.0` tag object `5e3c88ea6d937a89078a5e8f1b2a6fd0ea391a5c`):

- registered lazy detector ids include `eventmodeling`, `treeView`, `ishikawa`, `venn`, and
  `wardley`, plus new pinned 11.16 ids `swimlane`, `railroad`, `railroadEbnf`,
  `railroadAbnf`, `railroadPeg`, and `cynefin`.
- `treeView` is the upstream diagram id and config key; the pinned detector accepts the
  `treeView-beta` header.
- `swimlane` is the upstream diagram id and config key; the pinned detector accepts the
  `swimlane-beta` header.
- `railroad`, `railroadEbnf`, `railroadAbnf`, and `railroadPeg` are separate upstream detector ids
  with `railroad-beta`, `railroad-ebnf-beta`, `railroad-abnf-beta`, and `railroad-peg-beta`
  headers.
- `cynefin` is the upstream diagram id and config key; the pinned detector accepts
  `cynefin-beta` followed by whitespace, `:`, or end of input.
- diagram-local test sources:
  - `eventmodeling/eventmodeling.spec.ts`
  - `ishikawa/ishikawa.spec.ts`
  - `swimlanes/swimlanesDiagram.spec.ts`
  - `railroad/railroadDetector.spec.ts`
  - `railroad/ebnfDetector.spec.ts`
  - `railroad/abnfDetector.spec.ts`
  - `railroad/pegDetector.spec.ts`
  - `railroad/parser/railroadDiagram.spec.ts`
  - `railroad/parser/ebnfDiagram.spec.ts`
  - `railroad/parser/abnfDiagram.spec.ts`
  - `railroad/parser/pegDiagram.spec.ts`
  - `cynefin/cynefin.spec.ts`
  - `cynefin/cynefin.integration.spec.ts`
  - `venn/parser/venn.spec.ts`
  - `venn/vennRenderer.spec.ts`
  - `wardley/wardleyBuilder.spec.ts`
  - `wardley/wardleyParser.spec.ts`
- syntax docs exist for the pinned unsupported families:
  - `docs/syntax/eventmodeling.md`
  - `docs/syntax/ishikawa.md`
  - `docs/syntax/treeView.md`
  - `docs/syntax/venn.md`
  - `docs/syntax/swimlanes.md`
  - `docs/syntax/railroad.md`
  - `docs/syntax/cynefin.md`
  - `docs/syntax/wardley.md`

Approximate pinned source sizes used for priority shaping:

| Family | Parser | DB / builder | Renderer | Notes |
|---|---:|---:|---:|---|
| `treeView` | `16` lines | `69` lines | `114` lines | Lowest implementation surface; measurement still matters because labels use SVG bbox. |
| `ishikawa` | `45` lines | `79` lines | `468` lines | Pure tree geometry plus rough/getBBox phases. |
| `eventmodeling` | `25` lines | `602` lines | `138` lines | Semantic/state construction dominates. |
| `venn` | `110` lines | `116` lines | `336` lines | External Venn layout and optimizer behavior dominate risk. |
| `swimlane` | Flowchart parser reuse | Flowchart DB reuse | Flowchart renderer + swimlane layout/styles | Upstream `swimlanesDiagram.ts` calls `createFlowDiagram({ defaultLayout: 'swimlane' })`; local admission should reuse Flowchart semantics rather than fork a second parser. |
| `railroad` variants | `101` / `149` / `127` / `135` lines | `136` lines | `564` lines | Four parser dialects share a renderer/DB surface; parser strategy should preserve spans and recovery for LSP before choosing a generator. |
| `cynefin` | `18` lines | `86` lines | `378` lines | Parser/DB are modest, renderer policy and shape/layout parity dominate. |
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
