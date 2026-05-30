# ASCII Reference Implementation Expansion — TODO

Status: Active
Last updated: 2026-05-30

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

- [x] ARI-050 [owner=codex] [deps=ARI-010] [scope=crates/merman-ascii/src,xychart tests]
  Goal: Render xychart bar/line/mixed ASCII output from `RenderSemanticModel::XyChart`.
  Validation: `cargo nextest run -p merman-ascii xychart`
  Review: Self-review found no blocking findings. Chart scaling is deterministic and documented in
  `crates/merman-ascii/README.md`; the renderer consumes `XyChartDiagramRenderModel` and does not
  depend on SVG layout.
  Evidence: `crates/merman-ascii/src/xychart/`, `crates/merman-ascii/tests/xychart_model.rs`,
  `crates/merman-ascii/README.md`; gates recorded in `EVIDENCE_AND_GATES.md`.
  Handoff: DONE. Supports compact vertical bars, stair-step lines, mixed overlays, horizontal bars,
  title/axis text, inferred numeric x labels, empty charts, and ASCII/Unicode character sets.
  Legends, ANSI/color, multi-series spacing, and full-size terminal plot layout remain follow-on
  work.

## M4 — Flow/State Delta Triage

- [x] ARI-060 [owner=codex] [deps=ARI-010] [scope=crates/merman-ascii/src/graph,docs]
  Goal: Compare current graph renderer against `beautiful-mermaid` deltas and decide which
  behavior should be ported, rejected, or deferred.
  Validation: Documented gap matrix plus focused tests for any shipped behavior.
  Review: Self-review found no blocking findings. Parser-only features were rejected or deferred
  unless `merman-core` typed models preserve enough semantics; the current `RL` approximation in
  `beautiful-mermaid` is explicitly not ported because it would misrepresent Mermaid direction.
  Evidence: `crates/merman-ascii/FLOWCHART_SUPPORT.md`,
  `crates/merman-ascii/tests/flowchart_model.rs`, and graph stroke mapping in
  `crates/merman-ascii/src/graph/`; gates recorded in `EVIDENCE_AND_GATES.md`.
  Handoff: DONE. Thick edges were ported with ASCII/Unicode snapshots. BT true support, RL true
  support, subgraph direction overrides, multiline subgraph labels, ANSI/HTML color roles,
  class/style rendering, state diagram graph rendering, and additional uncommon shapes are
  documented as follow-on decisions.

## M5 — Integration And Closeout

- [x] ARI-070 [owner=codex] [deps=ARI-020,ARI-040,ARI-050] [scope=crates/merman-ascii,crates/merman,crates/merman-cli,README.md]
  Goal: Wire shipped diagram renderers through public APIs and docs without weakening existing
  feature gates.
  Validation: `cargo nextest run -p merman-ascii`; `cargo nextest run -p merman --features ascii`;
  `cargo nextest run -p merman-cli --features ascii`
  Review: Self-review found no blocking findings. `merman::ascii` now re-exports the shipped typed
  helpers and public-path tests cover class, ER, and XYChart through `merman` and `merman-cli`.
  Evidence: `crates/merman/src/ascii.rs`, `crates/merman/tests/ascii_api.rs`,
  `crates/merman-cli/tests/ascii_smoke.rs`, README support text, and
  `crates/merman-ascii/README.md`; gates recorded in `EVIDENCE_AND_GATES.md`.
  Handoff: DONE. Remaining unsupported diagram families, class/ER multi-relationship graph
  placement, true BT/RL flowchart transforms, style/color roles, and richer XYChart terminal layout
  should be split as follow-on lanes during ARI-080 closeout.

- [ ] ARI-080 [owner=planner] [deps=ARI-070] [scope=docs/workstreams/ascii-reference-implementation-expansion]
  Goal: Close the lane or split remaining work into narrower follow-ons.
  Validation: `verify-rust-workstream` records fresh final gate evidence.
  Review: `review-workstream` has no blocking findings.
  Evidence: `EVIDENCE_AND_GATES.md`, `WORKSTREAM.json`, `HANDOFF.md`
  Handoff: Summarize remaining risks and reference-source obligations.
