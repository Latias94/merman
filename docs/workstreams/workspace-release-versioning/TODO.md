# Workspace Release Versioning — TODO

Status: Active
Last updated: 2026-05-30

## M0 — Package Graph Freeze

- [x] WRV-010 [owner=planner] [deps=none] [scope=docs/workstreams/workspace-release-versioning]
  Goal: Freeze the package graph, current blockers, and evidence anchors.
  Validation: DESIGN.md, MILESTONES.md, EVIDENCE_AND_GATES.md, WORKSTREAM.json, and CONTEXT.jsonl exist and agree.
  Review: The lane must not imply that ABI work is incomplete; the blocker is release packaging.
  Evidence: EVIDENCE_AND_GATES.md
  Context: docs/workstreams/workspace-release-versioning/CONTEXT.jsonl
  Handoff: DONE. `merman-render` packages locally; `merman-bindings-core` and `merman-ffi` are
  blocked by crates.io dependency availability until a newer workspace version is published in
  dependency order.

## M1 — Publish Order And Version Decision

- [x] WRV-020 [owner=codex] [deps=WRV-010] [scope=docs/release,Cargo.toml]
  Goal: Document the publish order and choose the next workspace release version.
  Validation: publish-order doc exists and references the package evidence.
  Review: Version choice must account for immutable crates.io `0.6.0` packages.
  Evidence: docs/release publish-order doc.
  Context: docs/workstreams/workspace-release-versioning/CONTEXT.jsonl
  Handoff: DONE. Added `docs/release/PUBLISH_ORDER.md` and selected `0.7.0` as the next release
  target because the workspace added public binding crates/features and `0.6.0` is immutable on
  crates.io.

## M2 — Version Alignment

- [x] WRV-030 [owner=codex] [deps=WRV-020] [scope=Cargo.toml,Cargo.lock,crates/*/Cargo.toml]
  Goal: Align workspace package version and internal dependency version requirements to the chosen release version.
  Validation: cargo check -p merman-ffi && cargo check -p merman-uniffi
  Review: Do not weaken feature gates or path dependencies to force package verification.
  Evidence: version diff and cargo check output.
  Context: docs/workstreams/workspace-release-versioning/CONTEXT.jsonl
  Handoff: DONE. Updated workspace package version and internal workspace dependency requirements to
  `0.7.0`, including explicit `merman` and `merman-render` requirements in
  `merman-bindings-core`.

## M3 — Package Gate Matrix

- [ ] WRV-040 [owner=codex] [deps=WRV-030] [scope=docs/workstreams/workspace-release-versioning]
  Goal: Record package verification results in dependency order.
  Validation: cargo package commands recorded in EVIDENCE_AND_GATES.md.
  Review: Distinguish package file-list checks from full crates.io dependency verification.
  Evidence: package output for publishable crates.
  Context: docs/workstreams/workspace-release-versioning/CONTEXT.jsonl
  Handoff: Do not run `cargo publish`; this lane only prepares evidence and order.

## M4 — Closeout

- [ ] WRV-050 [owner=planner] [deps=WRV-040] [scope=docs/workstreams/workspace-release-versioning]
  Goal: Close or split remaining release blockers.
  Validation: verify-rust-workstream records fresh final gate evidence.
  Review: Platform lanes should know whether release packaging is ready or still blocked.
  Evidence: EVIDENCE_AND_GATES.md, WORKSTREAM.json, HANDOFF.md
  Handoff: Platform packaging lanes can start after version baseline is clear.
