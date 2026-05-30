# UniFFI Bindings — TODO

Status: Active
Last updated: 2026-05-30

## M0 — Scope And Evidence Freeze

- [x] UBI-010 [owner=planner] [deps=none] [scope=docs/workstreams/uniffi-bindings]
  Goal: Freeze the UniFFI lane around a shared safe facade plus minimal generated bindings.
  Validation: DESIGN.md, MILESTONES.md, EVIDENCE_AND_GATES.md, WORKSTREAM.json, and CONTEXT.jsonl exist and agree.
  Evidence: docs/workstreams/uniffi-bindings/DESIGN.md
  Context: docs/workstreams/uniffi-bindings/CONTEXT.jsonl
  Handoff: DONE. UniFFI 0.31.1 was confirmed from crates.io on 2026-05-30.

## M1 — Shared Binding Facade

- [x] UBI-020 [owner=codex] [deps=UBI-010] [scope=crates/merman-bindings-core,crates/merman-ffi]
  Goal: Extract options parsing, renderer setup, byte outputs, result codes, and error mapping into a safe shared facade.
  Validation: cargo nextest run -p merman-ffi && cargo check -p merman-bindings-core
  Review: `merman-ffi` public C symbols, result codes, and buffer ownership must not drift.
  Evidence: facade unit tests plus unchanged/passing FFI tests.
  Context: docs/workstreams/uniffi-bindings/CONTEXT.jsonl
  Handoff: DONE. Added `crates/merman-bindings-core` with safe render/parse/layout byte APIs,
  shared status/error payload mapping, options parsing, renderer setup, and RaTeX feature gating.
  Refactored `merman-ffi` to keep only raw pointer validation, panic containment, owned buffer
  transfer, and exported C symbols.

## M2 — Minimal UniFFI Crate

- [x] UBI-030 [owner=codex] [deps=UBI-020] [scope=crates/merman-uniffi]
  Goal: Add a minimal UniFFI crate exposing `render_svg`, `parse_json`, and `layout_json` over the shared facade.
  Validation: cargo check -p merman-uniffi && cargo test -p merman-uniffi
  Review: UniFFI API should be idiomatic but must not redefine the canonical C ABI protocol.
  Evidence: UniFFI crate tests and generated scaffolding build output.
  Context: docs/workstreams/uniffi-bindings/CONTEXT.jsonl
  Handoff: DONE. Added `crates/merman-uniffi` with a `MermanEngine` UniFFI object, `render_svg`,
  `parse_json`, and `layout_json` methods over `merman-bindings-core`, and a rich `MermanError`
  carrying `code`, `code_name`, and `message`.

## M3 — Generated Binding Smoke

- [ ] UBI-040 [owner=codex] [deps=UBI-030] [scope=crates/merman-uniffi,docs/bindings]
  Goal: Prove at least one generated binding path, or document the exact missing toolchain blocker.
  Validation: uniffi bindgen smoke command chosen by the implementer and recorded in EVIDENCE_AND_GATES.md.
  Review: Generated artifacts should not be committed unless they are intended package source.
  Evidence: command output and binding smoke notes.
  Context: docs/workstreams/uniffi-bindings/CONTEXT.jsonl
  Handoff: Split iOS/Android/Python packaging into platform lanes.

## M4 — Closeout

- [ ] UBI-050 [owner=planner] [deps=UBI-040] [scope=docs/workstreams/uniffi-bindings]
  Goal: Verify the lane and close or split platform/package follow-ons.
  Validation: verify-rust-workstream records fresh final gate evidence.
  Review: No blocking workstream or code-quality findings remain for the minimal UniFFI surface.
  Evidence: EVIDENCE_AND_GATES.md, WORKSTREAM.json, HANDOFF.md
  Handoff: Platform packaging lanes are follow-ons.
