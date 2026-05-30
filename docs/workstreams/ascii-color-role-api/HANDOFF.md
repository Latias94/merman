# ASCII Color Role API - Handoff

Status: Active
Last updated: 2026-05-30

## Current State

The workstream is active. ADR 0067 accepted the public color role API shape and the
`AsciiRenderOptions` migration. ACR-030 implemented the shared foreground-color substrate:
public color types, color options, role-aware `Canvas` storage, and forced ANSI/HTML finalizers.
Default plain output remains unchanged.

## Active Task

- Task ID: ACR-040
- Owner: unassigned
- Files: `crates/merman-ascii/src/graph`, `crates/merman-ascii/tests/flowchart_model.rs`
- Validation: `cargo nextest run -p merman-ascii flowchart_color`;
  `cargo nextest run -p merman-ascii flowchart`
- Status: TODO
- Review: plain flowchart snapshots must remain unchanged
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
- `set_role` and `write_text_role` exist for the next slice but no renderer assigns semantic roles
  yet.

## Blockers

- None.

## Next Recommended Action

- Assign flowchart roles for node text, node/group borders, edge lines, labels, arrowheads, and
  junctions using the role-aware canvas. Keep Mermaid style/class/linkStyle mapping deferred unless
  ACR-050 explicitly starts it.
