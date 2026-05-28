# Render Parser Registry - Handoff

Status: Complete
Last updated: 2026-05-28

## Current State

Workstream complete. `RenderDiagramRegistry` owns typed render parser lookup, and the engine keeps
JSON fallback behavior for custom diagrams.

## Completed Tasks

- Task ID: RPR-020
- Owner: codex
- Files:
  - `crates/merman-core/src/diagram/mod.rs`
  - `crates/merman-core/src/lib.rs`
  - `crates/merman-core/src/tests/misc.rs`
- Validation:
  - `cargo nextest run -p merman-core render_parser_registry`
- Status: DONE
- Evidence: `EVIDENCE_AND_GATES.md`

- Task ID: RPR-030
- Owner: codex
- Files:
  - `docs/workstreams/render-parser-registry/*`
- Validation:
  - package gates in `EVIDENCE_AND_GATES.md`
- Status: DONE
- Evidence: `EVIDENCE_AND_GATES.md`

## Decisions

- Keep JSON fallback through `DiagramRegistry`.
- Expose render registry accessors on `Engine` for symmetry with detector and JSON registries.
- Do not change date/time runtime behavior in this lane.

## Next Recommended Action

- Continue the broader fearless-refactor goal with the next high-leverage lane. Current candidates:
  text measurement ownership/cache cleanup, or a generated registry table if parser boilerplate
  starts growing again.
