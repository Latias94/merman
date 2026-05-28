# ASCII Renderer Compatibility Expansion - TODO

Status: Complete
Last updated: 2026-05-28

## M0 - Scope And Compatibility Policy

- [x] ACE-010 [owner=codex] [deps=none] [scope=docs/workstreams/ascii-renderer-compatibility-expansion,crates/merman-ascii/FLOWCHART_SUPPORT.md]
  Goal: Freeze the V1.1 compatibility policy for flowchart ASCII approximations and update the
  support matrix with planned supported/deferred behavior.
  Validation:
  - Workstream docs agree on target scope.
  - `FLOWCHART_SUPPORT.md` distinguishes supported, approximated, and unsupported constructs.
  Review: Confirm terminal approximations preserve Mermaid meaning.
  Evidence: `DESIGN.md` product policy and `crates/merman-ascii/FLOWCHART_SUPPORT.md` V1.1
  compatibility plan.
  Handoff: DONE. Next task: ACE-020.

## M1 - Flowchart Edge Semantics

- [x] ACE-020 [owner=codex] [deps=ACE-010] [scope=crates/merman-ascii/src/graph,crates/merman-ascii/tests/flowchart_model.rs,crates/merman-ascii/FLOWCHART_SUPPORT.md]
  Goal: Render common flowchart edge labels and edge variants instead of rejecting them.
  Validation:
  - `cargo nextest run -p merman-ascii flowchart`
  - `cargo nextest run -p merman-ascii graph::`
  Review: Check labels are visible, direction is preserved, and unsupported edge kinds still fail
  explicitly.
  Evidence: Focused parser/model tests and support matrix updates. `cargo nextest run -p
  merman-ascii flowchart`, `cargo nextest run -p merman-ascii graph::`, and `cargo check -p
  merman-ascii` passed on 2026-05-28.
  Handoff: DONE. Next task: ACE-030.

## M2 - Flowchart Shape Approximations

- [x] ACE-030 [owner=codex] [deps=ACE-010] [scope=crates/merman-ascii/src/graph,crates/merman-ascii/tests/flowchart_model.rs,crates/merman-ascii/FLOWCHART_SUPPORT.md]
  Goal: Render high-frequency non-rectangular node shapes with documented terminal approximations.
  Validation:
  - `cargo nextest run -p merman-ascii flowchart`
  - `cargo nextest run -p merman-ascii graph::`
  Review: Verify shape mapping is deterministic and does not claim SVG geometry parity.
  Evidence: Shape snapshot tests and support matrix updates. `cargo nextest run -p merman-ascii
  flowchart`, `cargo nextest run -p merman-ascii graph::`, and `cargo check -p merman-ascii`
  passed on 2026-05-28.
  Handoff: DONE. Next task: ACE-040.

## M3 - Flowchart Subgraphs

- [x] ACE-040 [owner=codex] [deps=ACE-020,ACE-030] [scope=crates/merman-ascii/src/graph,crates/merman-ascii/tests/flowchart_model.rs,crates/merman-ascii/FLOWCHART_SUPPORT.md]
  Goal: Render simple subgraphs as titled group boxes around supported member nodes.
  Validation:
  - `cargo nextest run -p merman-ascii flowchart`
  - `cargo nextest run -p merman-ascii graph::`
  Review: Verify containment and titles are preserved for simple LR/TD layouts; nested or routed
  edge cases may remain explicit follow-ons.
  Evidence: Subgraph parser/model tests and support matrix updates. `cargo nextest run -p
  merman-ascii flowchart`, `cargo nextest run -p merman-ascii graph::`, and `cargo check -p
  merman-ascii` passed on 2026-05-28.
  Handoff: DONE. Next task: ACE-050.

## M4 - Product Examples And CLI Smoke

- [x] ACE-050 [owner=codex] [deps=ACE-020,ACE-030,ACE-040] [scope=crates/merman-ascii/README.md,crates/merman-cli/tests,README.md,CHANGELOG.md]
  Goal: Add user-facing examples that demonstrate the expanded flowchart support through library
  and CLI entry points.
  Validation:
  - `cargo nextest run -p merman-cli --features ascii ascii`
  - `cargo check -p merman-cli --features ascii`
  Review: Existing SVG/raster CLI behavior must remain unchanged.
  Evidence: README examples, changelog entry, and CLI smoke coverage. `cargo nextest run -p
  merman-cli --features ascii ascii` and `cargo check -p merman-cli --features ascii` passed on
  2026-05-28.
  Handoff: DONE. Next task: ACE-060.

## M5 - Verification And Closeout

- [x] ACE-060 [owner=codex] [deps=ACE-050] [scope=docs/workstreams/ascii-renderer-compatibility-expansion]
  Goal: Run fresh focused gates, record evidence, and close this compatibility-expansion lane or
  split remaining work into follow-ons.
  Validation:
  - `cargo fmt --all --check`
  - `cargo nextest run -p merman-ascii`
  - `cargo nextest run -p merman --features ascii`
  - `cargo nextest run -p merman-cli --features ascii`
  - `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
  - `cargo clippy -p merman-cli --features ascii --all-targets -- -D warnings`
  - `git diff --check`
  Review: `verify-rust-workstream` followed by `close-workstream`.
  Evidence: `docs/workstreams/ascii-renderer-compatibility-expansion/EVIDENCE_AND_GATES.md`.
  Handoff: DONE. Workstream closed.
