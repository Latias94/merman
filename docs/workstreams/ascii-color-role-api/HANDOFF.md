# ASCII Color Role API - Handoff

Status: Active
Last updated: 2026-05-30

## Current State

The workstream is active. ADR 0067 accepted the public color role API shape and the
`AsciiRenderOptions` migration. ACR-030 implemented the shared foreground-color substrate:
public color types, color options, role-aware `Canvas` storage, and forced ANSI/HTML finalizers.
ACR-040 assigned flowchart semantic roles for nodes, groups, edges, labels, arrowheads, and routed
junctions. ACR-050 split broader family adoption into smaller lanes in `FAMILY_ADOPTION_PLAN.md`.
Default plain output remains unchanged.

## Active Task

- Task ID: ACR-051
- Owner: unassigned
- Files: `crates/merman-ascii/src/canvas.rs`, `crates/merman-ascii/src/relation_graph.rs`
- Validation: `cargo nextest run -p merman-ascii color canvas relation_graph`;
  `cargo fmt --all --check`
- Status: TODO
- Review: trailing-space trimming must stay byte-for-byte compatible in plain output
- Evidence: `EVIDENCE_AND_GATES.md`

## Decisions Since Last Update

- Default output should remain plain text and byte-for-byte compatible.
- `Auto` color mode should be opt-in because it depends on environment detection.
- The first implementation should be foreground-only roles; background/fill is a follow-on.
- `AsciiColorRole` should be non-exhaustive.
- `AsciiColorTheme` should have private fields and builder methods.
- Mermaid style mapping should not be bundled with the first role-canvas slice.
- ADR 0067 accepts a pre-1.0 `AsciiRenderOptions` migration: add color fields, keep `Copy`, add
  builder methods, and mark the struct `#[non_exhaustive]`.
- ACR-030 keeps diagram layout role-agnostic. The graph renderer now uses `Canvas::finish_with_options`
  only at final output boundaries, while transformed intermediate canvases still use plain
  finalization.
- ACR-040 moved flowchart drawing to semantic role helpers. `OutputTransform` now preserves canvas
  roles before redrawing transformed labels/titles.
- Flowchart roles cover node text, node borders, group borders/titles, edge lines, edge labels,
  arrowheads, and route junctions. Mermaid style/class/linkStyle mapping remains deferred.
- ACR-050 decided to split broader family adoption. Class and ER share relation graph string boxes
  and layered `Canvas` routing; sequence and XYChart use different string/char-grid output paths.
- The next substrate should support role-aware trimming before family-specific color writers land.

## Blockers

- None.

## Next Recommended Action

- Start ACR-051. Add a shared role-aware text/trim substrate for non-flowchart renderers, then use
  it in class/ER before moving to XYChart and sequence. Keep ACR-060 style/class/linkStyle mapping
  separate unless the planner explicitly prioritizes it.
