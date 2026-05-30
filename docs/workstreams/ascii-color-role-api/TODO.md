# ASCII Color Role API - TODO

Status: Active
Last updated: 2026-05-30

## M0 - API Design And Decision Point

- [x] ACR-010 [owner=codex] [deps=none] [scope=docs/workstreams/ascii-color-role-api]
  Goal: Draft the color role API boundary and implementation lane shape.
  Validation: `git diff --check -- docs/workstreams/ascii-color-role-api`
  Review: Confirm the API keeps default plain output byte-for-byte stable.
  Evidence: `DESIGN.md`
  Handoff: DONE. The next task should decide the public API migration path before code changes.

- [x] ACR-020 [owner=codex] [deps=ACR-010] [scope=docs/adr,docs/workstreams/ascii-color-role-api]
  Goal: Write an ADR for the color role API and the `AsciiRenderOptions` public-field migration.
  Validation: `git diff --check -- docs/adr/0067-ascii-color-role-api.md docs/workstreams/ascii-color-role-api`
  Review: Public API change must be reviewed before implementation.
  Evidence: `docs/adr/0067-ascii-color-role-api.md`
  Handoff: DONE. The workstream is active; ACR-030 can add the role-aware canvas and encoders.

## M1 - First Color Infrastructure Slice

- [x] ACR-030 [owner=codex] [deps=ACR-020] [scope=crates/merman-ascii/src/color.rs,crates/merman-ascii/src/options.rs,crates/merman-ascii/src/canvas.rs]
  Goal: Add public color types, role-aware canvas storage, and forced ANSI/HTML encoders without
  changing default output.
  Validation: `cargo nextest run -p merman-ascii color canvas`; `cargo fmt --all --check`
  Review: No diagram renderer should receive color-specific layout logic.
  Evidence: Unit tests for plain, truecolor, ansi256, ansi16, and HTML span encoding.
  Handoff: DONE. Flowchart can become the first semantic role writer after the encoder is stable.

## M2 - First Diagram Vertical Slice

- [x] ACR-040 [owner=codex] [deps=ACR-030] [scope=crates/merman-ascii/src/graph,crates/merman-ascii/tests/flowchart_model.rs]
  Goal: Assign flowchart roles for node text, node/group borders, edge lines, edge labels,
  arrowheads, and junctions.
  Validation: `cargo nextest run -p merman-ascii flowchart_color`; `cargo nextest run -p merman-ascii flowchart`
  Review: Plain flowchart snapshots must remain unchanged.
  Evidence: Forced truecolor and HTML parser-backed snapshots for a small flowchart.
  Handoff: DONE. Style/class/linkStyle mapping remains deferred unless ACR-050 explicitly starts it.

## M3 - Follow-On Adoption Plan

- [x] ACR-050 [owner=codex] [deps=ACR-040] [scope=crates/merman-ascii/src/sequence,crates/merman-ascii/src/class,crates/merman-ascii/src/er,crates/merman-ascii/src/xychart]
  Goal: Decide whether to adopt roles across all shipped diagram families or split smaller lanes.
  Validation: `git diff --check -- docs/workstreams/ascii-color-role-api crates/merman-ascii/FLOWCHART_SUPPORT.md crates/merman-ascii/README.md`
  Review: Sequence/class/ER/XYChart roles should preserve their current plain snapshots.
  Evidence: `FAMILY_ADOPTION_PLAN.md` and updated support docs.
  Handoff: DONE. Broader adoption is split; start ACR-051 before family-specific role writers.

- [x] ACR-051 [owner=codex] [deps=ACR-050] [scope=crates/merman-ascii/src/canvas.rs,crates/merman-ascii/src/relation_graph.rs]
  Goal: Add a shared role-aware text/trim substrate for non-flowchart renderers.
  Validation: `cargo nextest run -p merman-ascii color canvas relation_graph`; `cargo fmt --all --check`
  Review: Trimming trailing spaces must stay byte-for-byte compatible in plain output.
  Evidence: Unit tests for trimmed plain, truecolor, and HTML finalization plus a role-aware relation_graph box draw test.
  Handoff: Class and ER should adopt the substrate first because they share relation graph boxes.

- [ ] ACR-052 [owner=unassigned] [deps=ACR-051] [scope=crates/merman-ascii/src/class,crates/merman-ascii/src/er,crates/merman-ascii/tests]
  Goal: Adopt color roles for class and ER diagrams through the shared relation graph substrate.
  Validation: `cargo nextest run -p merman-ascii class_color er_color`; `cargo nextest run -p merman-ascii class er`
  Review: Existing class and ER plain snapshots must remain unchanged.
  Evidence: Forced truecolor and HTML parser-backed snapshots for class and ER.
  Handoff: Relationship markers, labels, and junctions should be role-aware before moving to charts.

- [ ] ACR-053 [owner=unassigned] [deps=ACR-051] [scope=crates/merman-ascii/src/xychart,crates/merman-ascii/tests]
  Goal: Adopt color roles for XYChart axes, text, bars, and line series.
  Validation: `cargo nextest run -p merman-ascii xychart_color`; `cargo nextest run -p merman-ascii xychart`
  Review: `ChartSeries(index)` should be used for plotted data and wrap by theme series length.
  Evidence: Forced truecolor and HTML parser-backed snapshots for bar and line charts.
  Handoff: Series role behavior should be stable before Mermaid style mapping uses direct colors.

- [ ] ACR-054 [owner=unassigned] [deps=ACR-051] [scope=crates/merman-ascii/src/sequence,crates/merman-ascii/tests]
  Goal: Adopt color roles for sequence participants, lifelines, activations, messages, notes, boxes,
  and control frames.
  Validation: `cargo nextest run -p merman-ascii sequence_color`; `cargo nextest run -p merman-ascii sequence`
  Review: Sequence plain golden comparisons must remain unchanged.
  Evidence: Forced truecolor and HTML parser-backed snapshots for messages, notes, and frames.
  Handoff: Background/fill interpretation for Mermaid `rect` and boxes remains deferred.

- [ ] ACR-060 [owner=unassigned] [deps=ACR-040] [scope=crates/merman-ascii/src/graph,crates/merman-ascii/FLOWCHART_SUPPORT.md]
  Goal: Design or implement Mermaid style mapping for flowchart `classDef`, `style`, and
  `linkStyle`.
  Validation: parser-backed tests from existing style fixtures.
  Review: Do not silently misrepresent unsupported CSS properties.
  Evidence: Style resolver tests and support matrix updates.
  Handoff: Background/fill color remains a separate decision unless explicitly accepted.
