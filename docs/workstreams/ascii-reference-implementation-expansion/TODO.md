# ASCII Reference Implementation Expansion — TODO

Status: Active
Last updated: 2026-05-29

## M0 — Reference Intake And Provenance

- [x] ARI-010 [owner=planner] [deps=none] [scope=README.md,crates/merman-ascii,tools/upstreams,docs/workstreams/ascii-reference-implementation-expansion]
  Goal: Register `mermaid-ascii` and `beautiful-mermaid` as reference implementations with license
  notices, upstream pins, and a model-driven usage boundary.
  Validation: `git diff --check`; docs and license files agree on source URLs, commits, and license
  names.
  Evidence: `crates/merman-ascii/README.md`, `crates/merman-ascii/LICENSES/beautiful-mermaid-MIT.txt`, `tools/upstreams/REPOS.lock.json`
  Handoff: DONE. No runtime behavior changed.

## M1 — Class Diagram ASCII

- [x] ARI-020 [owner=codex] [deps=ARI-010] [scope=crates/merman-ascii/src,class tests]
  Goal: Render a first classDiagram ASCII/Unicode slice from `RenderSemanticModel::Class`.
  Validation: `cargo nextest run -p merman-ascii class`
  Review: Self-review found no blocking findings; broader planner review can still inspect ARI-030
  relationship expansion before the class milestone closes.
  Evidence: `crates/merman-ascii/src/class/`, `crates/merman-ascii/tests/class_model.rs`,
  `crates/merman-ascii/README.md`; gates recorded in `EVIDENCE_AND_GATES.md`.
  Handoff: DONE. The slice supports class boxes, members, methods, Unicode/ASCII borders, and one
  solid extension relationship. Labels and other relationship kinds are explicitly unsupported.

- [x] ARI-030 [owner=codex] [deps=ARI-020] [scope=crates/merman-ascii/src,class tests]
  Goal: Expand class ASCII relationships using `beautiful-mermaid` as a reference for inheritance,
  dependency, aggregation, composition, labels, and arrow orientation.
  Validation: `cargo nextest run -p merman-ascii class`
  Review: Relationship constants are mapped from `merman-core` typed constants and `RelationShape`;
  no Mermaid text reparsing was added.
  Evidence: `crates/merman-ascii/src/class/render.rs`, `crates/merman-ascii/tests/class_model.rs`,
  `crates/merman-ascii/README.md`; gates recorded in `EVIDENCE_AND_GATES.md`.
  Handoff: DONE. Supports single-relationship layouts for extension, dependency, aggregation,
  composition, labels, arrow orientation, dotted dependency lines, and Unicode composition markers.
  Multiple class relationships and richer graph layout remain follow-on work.

## M2 — ER Diagram ASCII

- [x] ARI-040 [owner=codex] [deps=ARI-010] [scope=crates/merman-ascii/src,er tests]
  Goal: Render ER entity boxes, attributes, relationship labels, and crow's-foot markers from
  `RenderSemanticModel::Er`.
  Validation: `cargo nextest run -p merman-ascii er`
  Review: Cardinality and identifying mappings are read from the typed `ErRelSpecRenderModel`; no
  Mermaid text reparsing was added.
  Evidence: `crates/merman-ascii/src/er/`, `crates/merman-ascii/tests/er_model.rs`,
  `crates/merman-ascii/README.md`; gates recorded in `EVIDENCE_AND_GATES.md`.
  Handoff: DONE. Supports entity boxes, attributes, relationship labels, identifying and
  non-identifying lines, and common cardinality markers for single-relationship layouts. Multiple
  ER relationship graph layout and styling/classes remain follow-on work.

## M3 — XYChart ASCII

- [ ] ARI-050 [owner=unassigned] [deps=ARI-010] [scope=crates/merman-ascii/src,xychart tests]
  Goal: Render xychart bar/line/mixed ASCII output from `RenderSemanticModel::XyChart`.
  Validation: `cargo nextest run -p merman-ascii xychart`
  Review: Keep chart scaling deterministic and documented; do not depend on SVG layout.
  Evidence: XYChart ASCII snapshots for vertical bars, lines, mixed plots, horizontal orientation,
  titles, axes, and edge cases.
  Handoff: Split ANSI/color legends if they obscure the plain-text proof.

## M4 — Flow/State Delta Triage

- [ ] ARI-060 [owner=unassigned] [deps=ARI-010] [scope=crates/merman-ascii/src/graph,docs]
  Goal: Compare current graph renderer against `beautiful-mermaid` deltas and decide which
  behavior should be ported, rejected, or deferred.
  Validation: Documented gap matrix plus focused tests for any shipped behavior.
  Review: Reject parser-only features that cannot be expressed through `merman-core` typed models.
  Evidence: Updated `FLOWCHART_SUPPORT.md` and test references.
  Handoff: Candidate deltas include BT/RL approximations, thick edges, multiline subgraph labels,
  and ANSI/HTML color roles.

## M5 — Integration And Closeout

- [ ] ARI-070 [owner=unassigned] [deps=ARI-020,ARI-040,ARI-050] [scope=crates/merman-ascii,crates/merman,crates/merman-cli,README.md]
  Goal: Wire shipped diagram renderers through public APIs and docs without weakening existing
  feature gates.
  Validation: `cargo nextest run -p merman-ascii`; `cargo nextest run -p merman --features ascii`;
  `cargo nextest run -p merman-cli --features ascii`
  Review: Public API and docs review before closeout.
  Evidence: Support matrices and examples for shipped diagram types.
  Handoff: Split any remaining diagram family into a follow-on lane.

- [ ] ARI-080 [owner=planner] [deps=ARI-070] [scope=docs/workstreams/ascii-reference-implementation-expansion]
  Goal: Close the lane or split remaining work into narrower follow-ons.
  Validation: `verify-rust-workstream` records fresh final gate evidence.
  Review: `review-workstream` has no blocking findings.
  Evidence: `EVIDENCE_AND_GATES.md`, `WORKSTREAM.json`, `HANDOFF.md`
  Handoff: Summarize remaining risks and reference-source obligations.
