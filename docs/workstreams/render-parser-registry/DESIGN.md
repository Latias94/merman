# Render Parser Registry

Status: Complete
Last updated: 2026-05-28

## Why This Lane Exists

The typed render parser path still used a hand-maintained `Engine::parse_render_semantic_model`
match after the typed model metadata refactor. That kept every typed diagram parser and alias in a
large engine method instead of a registry boundary.

This lane moves typed render parser dispatch behind a registry while preserving JSON fallback
behavior for custom or plugin-style diagrams.

## Target State

- `RenderDiagramRegistry` owns typed render parser lookup.
- `Engine` has a default render parser registry alongside detector and JSON semantic registries.
- `Engine::parse_render_semantic_model` performs a registry lookup, then falls back to
  `DiagramRegistry` JSON parsing.
- Public parse APIs and renderer behavior stay unchanged.

## In Scope

- Add `RenderSemanticParser` and `RenderDiagramRegistry`.
- Register Mermaid 11.12.2 typed render parsers and aliases.
- Expose render registry accessors on `Engine`.
- Add focused tests for typed alias lookup and JSON fallback.

## Out Of Scope

- Changing detector behavior.
- Changing `RenderSemanticModel` enum variants.
- Changing JSON semantic parser registration.
- Reworking runtime date/time handling.
- Generating parser tables from macros.

## Closeout Condition

- Focused registry tests pass.
- `merman-core` and `merman-render` package gates pass.
- Workstream evidence records any behavior deliberately left unchanged.
