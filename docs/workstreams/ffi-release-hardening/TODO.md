# FFI Release Hardening — TODO

Status: Closed with packaging concern
Last updated: 2026-05-30

## M0 — Scope And Evidence Freeze

- [x] FRH-010 [owner=planner] [deps=none] [scope=docs/workstreams/ffi-release-hardening]
  Goal: Freeze problem, target state, non-goals, and evidence anchors.
  Validation: DESIGN.md, MILESTONES.md, EVIDENCE_AND_GATES.md, WORKSTREAM.json, and CONTEXT.jsonl exist and agree.
  Evidence: docs/workstreams/ffi-release-hardening/DESIGN.md
  Context: docs/workstreams/ffi-release-hardening/CONTEXT.jsonl
  Handoff: DONE. This lane is active and ready for focused implementation.

## M1 — Real C Consumer Smoke

- [x] FRH-020 [owner=codex] [deps=FRH-010] [scope=crates/merman-ffi]
  Goal: Add a realistic C consumer smoke test that calls the public ABI, checks success/error payloads, and frees every returned buffer.
  Validation: cargo nextest run -p merman-ffi c_consumer_smoke
  Review: Header declarations, result ownership, and C compile/link assumptions must match `include/merman.h`.
  Evidence: `crates/merman-ffi/tests` C smoke source and Rust test harness.
  Context: docs/workstreams/ffi-release-hardening/CONTEXT.jsonl
  Handoff: DONE. Added `tests/c_consumer_smoke.c` plus a Rust test that compiles the C source into
  a dynamic library, loads it, calls the ABI via C function pointers, verifies success/error
  payloads, and frees every returned buffer.

## M2 — Package And Docs Entry

- [x] FRH-030 [owner=codex] [deps=FRH-020] [scope=crates/merman-ffi,README.md,docs/bindings]
  Goal: Make the FFI crate package-checkable and document the C ABI entry point from project docs.
  Validation: cargo package -p merman-ffi --allow-dirty
  Review: Publishing metadata should not hide the public header or point users only at Rust APIs.
  Evidence: README/doc changes and package command output.
  Context: docs/workstreams/ffi-release-hardening/CONTEXT.jsonl
  Handoff: DONE_WITH_CONCERNS. `cargo package -p merman-ffi --allow-dirty --list` includes the
  crate README, header, source, and smoke tests. Full package verification is blocked until the
  workspace publishes a newer `merman-render` version containing `ratex-math`.

## M3 — Closeout

- [x] FRH-040 [owner=planner] [deps=FRH-030] [scope=docs/workstreams/ffi-release-hardening]
  Goal: Verify the lane and close or split follow-ons.
  Validation: verify-rust-workstream records fresh final gate evidence.
  Review: No blocking workstream or code-quality findings remain for the hardening scope.
  Evidence: EVIDENCE_AND_GATES.md, WORKSTREAM.json, HANDOFF.md
  Handoff: DONE_WITH_CONCERNS. Lane is closed with package verification deferred to a workspace
  release-version/publish-order follow-on. UniFFI remains the expected next implementation lane.
