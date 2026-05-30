# ASCII Flowchart Direction Transform - TODO

Status: Active
Last updated: 2026-05-30

## M0 - Scope And Evidence Freeze

- [x] AFDT-010 [owner=codex] [deps=none] [scope=docs/workstreams/ascii-flowchart-direction-transform]
  Goal: Freeze the BT/RL flowchart direction problem, target state, and evidence anchors.
  Validation: `git diff --check -- docs/workstreams/ascii-flowchart-direction-transform`
  Review: Confirm BT/RL are the only root-direction targets and that color/style, subgraph
  overrides, and state diagrams stay out of scope.
  Evidence: `DESIGN.md`
  Handoff: AFDT-020 starts with parser-backed BT/RL tests.

## M1 - Direction Contract Tests

- [ ] AFDT-020 [owner=unassigned] [deps=AFDT-010] [scope=crates/merman-ascii/tests]
  Goal: Add parser-backed flowchart tests that describe BT and RL root-direction output instead of
  the current unsupported-feature diagnostic.
  Validation: `cargo nextest run -p merman-ascii flowchart`
  Review: Tests must exercise public `render_model` behavior and fail red on the old
  non-LR/TD-direction diagnostic.
  Evidence: BT/RL flowchart direction tests.
  Handoff: AFDT-030 implements the transform.

## M2 - Root Direction Transform

- [ ] AFDT-030 [owner=unassigned] [deps=AFDT-020] [scope=crates/merman-ascii/src/graph,crates/merman-ascii/src/flowchart,crates/merman-ascii/tests]
  Goal: Implement a real BT vertical flip and RL horizontal mirror for flowchart ASCII output.
  Validation: `cargo nextest run -p merman-ascii flowchart`; `cargo clippy -p merman-ascii --all-targets -- -D warnings`
  Review: Keep LR/TD stable and avoid parser or core changes.
  Evidence: Flowchart BT/RL render through the public ASCII surface.
  Handoff: AFDT-040 updates docs and closes or splits remaining flowchart direction work.

## M3 - Docs And Closeout

- [ ] AFDT-040 [owner=planner] [deps=AFDT-030] [scope=docs/workstreams/ascii-flowchart-direction-transform,crates/merman-ascii/FLOWCHART_SUPPORT.md,README.md]
  Goal: Update support docs, run final gates, and close or split follow-on flowchart work.
  Validation: `cargo nextest run -p merman-ascii`; `cargo fmt --all --check`; `git diff --check`
  Review: Use `review-workstream` and `verify-rust-workstream` before closeout.
  Evidence: README/support docs and final handoff.
  Handoff: Lane closes or names narrower follow-ons.
