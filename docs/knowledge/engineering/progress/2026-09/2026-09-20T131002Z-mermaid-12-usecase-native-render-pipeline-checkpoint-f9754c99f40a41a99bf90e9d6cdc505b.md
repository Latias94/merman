---
type: "Work Progress"
title: "Mermaid 12 Usecase native render pipeline checkpoint"
description: "Usecase layout and SVG pipeline tested; source gaps and family admission remain open."
timestamp: 2026-09-20T13:10:02Z
record_id: "f9754c99f40a41a99bf90e9d6cdc505b"
producer_id: "codex-mermaid12"
run_id: "session-01a0bd10"
related_plan: "docs/plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md"
git_branch: "refactor/mermaid-12-alignment"
git_commit: "65de04b56"
supersedes: "05764f346b3b4026a526866ddfb1d777"
---

# Summary

GOAL U1–U15 remains active and incomplete. Target alpha.7 contains unreleased main plus Mermaid 12
alignment. Package versions remain alpha.6. Selected reference remains 11.17.2 until atomic U15
promotion; the staged runtime uses the pinned Mermaid 12 source and verified package graph. No
push, PR, publication, or merge.

# Implemented

Commit `65de04b56` integrates a native Usecase prepared artifact through `family.rs`, layout JSON and
SVG dispatch. Family catalog admission is deliberately still absent: core renderer metadata remains
`None` until the remaining source contracts are closed.

- Measured actor variants, independent/folded stereotypes, ellipse/rect/business use cases, notes,
  JSON tables with ordered flattening, and package/rect boundaries.
- Real Dagre/ELK selection, shared capability policy, ELK options/operation seed and work controls.
- Separate raw provider routes and prepared drawing paths. Family-local ELK geometry ports shape
  intersections, group clipping, orthogonal ellipse exits, tiny-node alignment, marker stubs and
  terminal jog crossing checks. Empty sections retain empty provider points and derive paint paths
  and midpoint labels separately. Worker `usecase_layout` completed this source-backed geometry.
- Sanitization occurs before measurement/folding. Original sanitized labels are retained to compute
  accessibility before stereotypes are folded. Relationship endpoints use Markdown plain text,
  confirmed against the actual Mermaid 12 runtime.
- Role colors, optional participant palette rotation, independent boundary palette numbering,
  original class/style and edge animation class transport. CSS uses raw XML escaping: the shared
  diagram-text escape helper decodes Mermaid #entities and would corrupt hex colors followed by
  semicolons. A representative palette test caught and verifies this correction.
- Shared edge corner/marker-offset functions moved out of Flowchart, preserving its wrappers.
  Usecase chooses the source curve (ELK routed=rounded, missing=linear, Dagre=flowchart.curve).
  Added D3 bumpX via the shared axis implementation and enabled it in Flowchart selection too.
- Diagram title baseline now follows upstream insertTitle at y=0, with existing measured title
  bounds instead of fixed extra spacing.

# Verification

- `cargo nextest run -p merman-render --features layout-elk --test usecase_svg_test --lib -E
  'test(usecase) | test(svg::parity::flowchart::edge_geom::line_with_offset) |
  test(svg::parity::flowchart::edge_geom::curve_path)'`: 19 passed; nextest run
  `4b63f47e-6001-46ed-b731-3bc14e0c361d`.
- `cargo check -p merman-render --no-default-features`: passed.
- Changed-file rustfmt check and `git diff --check`: passed.
- Temporary oracle `target/mermaid-12-admission/usecase-render-probe.cjs` renders the complex and
  sanitizer fixture with Mermaid 12 and htmlLabels true/false. Its JSON captures source SVG and
  accessibility/class/path observations. This is exploratory evidence, not golden admission.
  Chrome headless shell and the previously verified staged runtime were reused.
- D3-shape from the same runtime directly produced the bumpX/Y expected paths used in the focused
  shared-curve regression test. No new package installation, signature verification or full suite.

# Remaining Work and Next Action

1. ELK `spreadPorts`: upstream sets node-level `elk.portAlignment.default=CENTER` for Usecase
   ellipse/business. Lower LNode already has `port_alignment`, but `ElkInputNode` in
   `merman-elk-layered/src/importer.rs` and public `merman-layout-elk::Node` do not transport it.
   Add a typed option through both importer paths in `merman-layout-elk/src/lib.rs`, use it for
   ellipse plans, and verify a distinguishing multi-edge source fixture. Do not spread endpoints
   artificially after layout. Public Node constructors and importer test constructors need updates.
2. Dagre compound integration currently uses direct compound layout, not Mermaid's nested graph
   extraction and full boundary/title space projection. Port the actual source path.
3. SVG remains an initial source-backed pipeline, not full DOM admission. Close label/style
   projection (including text styles and edge fonts), root font custom properties, host measurement
   carrier/reuse ownership, raw SVG bounds, handDrawn shape behavior, and exact group/label/marker
   structure using family-local comparisons. Do not bulk-accept current output.
4. Independent parser/layout/render review still required. Parser ordered JSON uses serde_json's
   default recursion bound; assess source acceptance versus explicit resource policy. Check scalar
   property order, deeply nested/duplicate keys and externally constructed typed-model validation.
5. Source renderer probe showed the disconnected JSON table absent in its second HTML render;
   investigate reused-runtime state/ID effects before interpreting that as a family behavior.
6. Other planned Mermaid 12 units remain open, especially Agentflow, broad existing-family/theme
   deltas, editor/FFI/Web WASM/Playground, golden refresh and final coherent bundle/version promotion.
   The successful narrow checks above do not close those requirements.

# Source Identity

Mermaid checkout: `98a0945418c76238f15df2afaddbba4272656c3b`.
Trusted baseline: `54d257aa8be7ae9ab1332d1b257205b60b43dbbd`.
Usecase references: `usecaseDb.ts`, `usecaseRenderer.ts`, `styles.ts` under
`repo-ref/mermaid/packages/mermaid/src/diagrams/usecase`; shared shapes are under
`rendering-util/rendering-elements/shapes` (not src/rendering-elements).

Use UTF-8 explicitly when writing Python files on Windows. One failed GBK write truncated the
untracked measurement file during this turn; it was recovered from the exact preceding tool read
before applying the edit. The final recovered contents compiled and passed the focused tests.
