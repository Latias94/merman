# Post Alpha.2 Fearless Refactor — TODO

Status: Active
Last updated: 2026-06-08

## M0 — Plan Record

- [x] PA2R-010 [owner=codex] [deps=none] [scope=docs/workstreams/post-alpha2-fearless-refactor]
  Goal: Record the post-alpha.2 fearless refactor priorities and the local fallback because `ce-plan` is unavailable.
  Validation: `git diff --check -- docs/workstreams/post-alpha2-fearless-refactor`
  Evidence: `DESIGN.md`, `WORKSTREAM.json`

## M1 — Binding Render Request Module

- [x] PA2R-020 [owner=codex] [deps=PA2R-010] [scope=crates/merman-bindings-core/src/render.rs,crates/merman-bindings-core/src/render]
  Goal: Move binding render options, renderer construction, SVG pipeline construction, request execution, and render error classification behind a deeper request Module used by one-shot functions and cached engines.
  Validation: `cargo nextest run -p merman-bindings-core`; `cargo nextest run -p merman-ffi render_svg`; `cargo fmt --all --check`
  Review: Keep FFI/platform ABI stable; the refactor should reduce caller knowledge rather than only move code.
  Evidence: `crates/merman-bindings-core/src/render/request.rs`; focused gates passed on 2026-06-08.

## M2 — Next Architecture Slice

- [ ] PA2R-030 [owner=codex] [deps=PA2R-020] [scope=crates/merman-core/src/family.rs,crates/merman-core/src/detect,crates/merman-core/src/diagram]
  Goal: Reassess Diagram Family Facts after bindings cleanup and deepen the next highest-leverage projection without changing public JSON output.
  Validation: `cargo nextest run -p merman-core registry`; `cargo nextest run -p merman-core detect`; `cargo run -p xtask -- check-alignment`
