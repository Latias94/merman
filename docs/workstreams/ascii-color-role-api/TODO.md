# ASCII Color Role API - TODO

Status: Draft
Last updated: 2026-05-30

## M0 - API Design And Decision Point

- [x] ACR-010 [owner=codex] [deps=none] [scope=docs/workstreams/ascii-color-role-api]
  Goal: Draft the color role API boundary and implementation lane shape.
  Validation: `git diff --check -- docs/workstreams/ascii-color-role-api`
  Review: Confirm the API keeps default plain output byte-for-byte stable.
  Evidence: `DESIGN.md`
  Handoff: DONE. The next task should decide the public API migration path before code changes.

- [ ] ACR-020 [owner=unassigned] [deps=ACR-010] [scope=docs/adr,crates/merman-ascii/src/options.rs,crates/merman-ascii/src/lib.rs]
  Goal: Write an ADR for the color role API and the `AsciiRenderOptions` public-field migration.
  Validation: ADR accepted or workstream remains draft.
  Review: Public API change must be reviewed before implementation.
  Evidence: New ADR plus updated `DESIGN.md` if the accepted API differs.
  Handoff: Do not implement color fields until this decision is made.

## M1 - First Color Infrastructure Slice

- [ ] ACR-030 [owner=unassigned] [deps=ACR-020] [scope=crates/merman-ascii/src/color.rs,crates/merman-ascii/src/options.rs,crates/merman-ascii/src/canvas.rs]
  Goal: Add public color types, role-aware canvas storage, and forced ANSI/HTML encoders without
  changing default output.
  Validation: `cargo nextest run -p merman-ascii color canvas`; `cargo fmt --all --check`
  Review: No diagram renderer should receive color-specific layout logic.
  Evidence: Unit tests for plain, truecolor, ansi256, ansi16, and HTML span encoding.
  Handoff: Flowchart can become the first semantic role writer after the encoder is stable.

## M2 - First Diagram Vertical Slice

- [ ] ACR-040 [owner=unassigned] [deps=ACR-030] [scope=crates/merman-ascii/src/graph,crates/merman-ascii/tests/flowchart_model.rs]
  Goal: Assign flowchart roles for node text, node/group borders, edge lines, edge labels,
  arrowheads, and junctions.
  Validation: `cargo nextest run -p merman-ascii flowchart_color`; `cargo nextest run -p merman-ascii flowchart`
  Review: Plain flowchart snapshots must remain unchanged.
  Evidence: Forced truecolor and HTML parser-backed snapshots for a small flowchart.
  Handoff: Style/class/linkStyle mapping remains deferred unless ACR-050 explicitly starts it.

## M3 - Follow-On Adoption Plan

- [ ] ACR-050 [owner=unassigned] [deps=ACR-040] [scope=crates/merman-ascii/src/sequence,crates/merman-ascii/src/class,crates/merman-ascii/src/er,crates/merman-ascii/src/xychart]
  Goal: Decide whether to adopt roles across all shipped diagram families or split smaller lanes.
  Validation: family-specific nextest filters for each adopted renderer.
  Review: Sequence/class/ER/XYChart roles should preserve their current plain snapshots.
  Evidence: Updated support docs and per-family color tests.
  Handoff: Open narrower lanes if adoption would be too broad for one task.

- [ ] ACR-060 [owner=unassigned] [deps=ACR-040] [scope=crates/merman-ascii/src/graph,crates/merman-ascii/FLOWCHART_SUPPORT.md]
  Goal: Design or implement Mermaid style mapping for flowchart `classDef`, `style`, and
  `linkStyle`.
  Validation: parser-backed tests from existing style fixtures.
  Review: Do not silently misrepresent unsupported CSS properties.
  Evidence: Style resolver tests and support matrix updates.
  Handoff: Background/fill color remains a separate decision unless explicitly accepted.
