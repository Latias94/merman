# ASCII Flowchart Subgraph Title Wrapping — TODO

Status: Closed
Last updated: 2026-05-30

## M0 — Scope And Evidence Freeze

- [x] WS-010 [owner=codex] [deps=none] [scope=docs/workstreams/ascii-flowchart-subgraph-title-wrapping]
  Goal: Freeze the long-title wrapping problem, target state, and evidence anchors.
  Validation: DESIGN.md, MILESTONES.md, EVIDENCE_AND_GATES.md, WORKSTREAM.json, and CONTEXT.jsonl exist and agree.
  Evidence: docs/workstreams/ascii-flowchart-subgraph-title-wrapping/DESIGN.md
  Context: docs/workstreams/ascii-flowchart-subgraph-title-wrapping/CONTEXT.jsonl
  Handoff: DONE. Implementation continued through AFSW-020.

## M1 — First Vertical Proof

- [x] AFSW-020 [owner=codex] [deps=WS-010] [scope=crates/merman-ascii/tests,crates/merman-ascii/src/graph]
  Goal: Add a red parser-backed contract for automatic long-title wrapping and a direct-model contract if needed.
  Validation: cargo nextest run -p merman-ascii flowchart_parser_long_subgraph_title_wraps_to_multiple_rows
  Review: review-workstream before accepting completion.
  Evidence: crates/merman-ascii/tests/flowchart_model.rs or a new focused flowchart test file.
  Context: docs/workstreams/ascii-flowchart-subgraph-title-wrapping/CONTEXT.jsonl plus the current flowchart support docs.
  Handoff: DONE. The parser-backed contract failed red against raw one-line title expansion and passes after wrapping.

## M2 — Integration And Docs

- [x] AFSW-030 [owner=codex] [deps=AFSW-020] [scope=crates/merman-ascii/src/graph,crates/merman-ascii/FLOWCHART_SUPPORT.md,crates/merman-ascii/README.md,README.md]
  Goal: Implement wrapped subgraph-title rendering and update support docs.
  Validation: cargo nextest run -p merman-ascii flowchart
  Review: review-workstream for workstream compliance and code quality.
  Evidence: crates/merman-ascii/FLOWCHART_SUPPORT.md
  Context: docs/workstreams/ascii-flowchart-subgraph-title-wrapping/CONTEXT.jsonl
  Handoff: DONE. `GraphLabel::wrapped` feeds layout/draw, support docs list wrapped subgraph titles as shipped.

## M3 — Closeout

- [x] AFSW-040 [owner=codex] [deps=AFSW-030] [scope=docs/workstreams/ascii-flowchart-subgraph-title-wrapping]
  Goal: Close the lane or split a narrower follow-on.
  Validation: verify-rust-workstream records fresh final gate evidence.
  Review: review-workstream has no blocking findings.
  Evidence: EVIDENCE_AND_GATES.md, WORKSTREAM.json
  Handoff: DONE. Lane closed; subgraph direction overrides and broader text wrapping remain separate follow-ons.
