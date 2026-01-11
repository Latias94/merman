# ADR-0003: Workspace and Crate Boundaries

## Status

Accepted

## Context

`merman` is intended to be a headless, reusable implementation of Mermaid that can be integrated
into many environments (CLI, servers, GUI apps, WebAssembly, etc.). We need crate boundaries that
avoid later refactors and keep responsibilities clear.

## Decision

- Use a Cargo workspace with the following initial crates:
  - `crates/merman-core`: core parsing pipeline (preprocess + detect + parse) and shared types.
  - `crates/merman`: public facade/re-export crate for end users.
  - `crates/merman-cli`: CLI entry point (initially scaffolded).
- Future crates are expected, but must be optional and layered:
  - `merman-model`: diagram semantic models (if it grows beyond `merman-core`).
  - `merman-render-svg`: pure SVG rendering (no DOM).
  - `merman-layout-*`: layout engines (dagre/elk equivalents), feature-gated.
  - `merman-wasm`: WASM bindings.

## Consequences

- `merman-core` must stay “headless-first” and avoid UI/runtime assumptions.
- Rendering and layout are separate crates so that consumers can choose their own stack.
