# Resvg-Safe SVG Output Pipeline - Handoff

Status: Active
Last updated: 2026-05-28

## Current State

The workstream and ADR are open. The first focused fallback correctness task is implemented and
verified: readable fallback text now splits literal `\n` inside `<foreignObject>` labels into
separate overlay text lines.

## Current Task

- Task ID: RSO-030
- Owner: codex
- Files:
  - `crates/merman-render/src/svg`
  - `crates/merman/src/lib.rs`
- Goal: Introduce `SvgPipeline` presets without changing default parity rendering.
- Validation:
  - `cargo nextest run -p merman-render fallback`
  - `cargo nextest run -p merman`
  - `cargo fmt -p merman-render -p merman -- --check`
- Status: TODO

## Decisions

- Keep parity output as the default.
- Build consumer cleanup as an explicit pipeline.
- Expose a string/Cow custom postprocessor first; keep event-stream internals private.
- Do not copy Zed GPL implementation.

## Next Step

Start RSO-030 by moving readable fallback behavior behind a pipeline preset while keeping existing
public helpers as compatibility wrappers.
