# ADR 0042: Rendering Strategy (SVG) for Mermaid Parity

## Status

Superseded by ADR 0043

## Context

`merman` targets 1:1 parity with Mermaid `@11.12.2`. We already have a headless parser that produces
a stable semantic JSON model and a fixture-based snapshot parity harness.

The remaining large milestone is rendering (primarily SVG) and layout. Upstream Mermaid rendering
depends on multiple mature JavaScript libraries and engines (e.g. dagre, elkjs, d3 and a large set
of diagram-specific renderers). A fully Rust-native reimplementation that matches Mermaid output
pixel-for-pixel is a multi-year effort with high risk of behavioral drift.

We need a strategy that:

- preserves the parity goal (the upstream supports it ⇒ `merman` supports it),
- allows headless usage (no browser required),
- is practical to ship and test in CI,
- keeps a path open for future Rust-native components where they make sense.

## Options

### A) Rust-native rendering and layout (pure Rust)

Implement SVG generation, layout engines (dagre/ELK equivalents) and per-diagram renderers in Rust.

Pros:
- pure Rust distribution story
- no JS runtime dependency

Cons:
- very large scope and long timeline
- hardest to keep 1:1 parity with Mermaid rendering behavior
- significant maintenance cost across Mermaid upgrades

### B) JS-backed renderer embedded in Rust (canonical parity path)

Embed a JavaScript runtime (e.g. QuickJS/V8 via a Rust binding) and run the upstream Mermaid
rendering pipeline pinned to `@11.12.2` to produce SVG. The Rust API stays headless and deterministic.

Pros:
- highest chance of true 1:1 rendering parity
- fastest path to “complete Mermaid support” for SVG
- can reuse upstream bugfix knowledge and test vectors

Cons:
- introduces a JS runtime dependency and packaging complexity
- requires careful sandboxing and resource limits
- bundle/build steps for the pinned Mermaid JS artifacts

### C) External renderer process (node/deno) orchestrated by Rust

Spawn an external process to render (similar to `mmdc` usage), and parse stdout/stderr.

Pros:
- simplest implementation
- parity depends on upstream renderer

Cons:
- weakest distribution story (requires external runtime)
- harder to make deterministic and cross-platform in CI
- more operational surface area

## Decision

This ADR explored rendering strategies. The project direction is now a pure-Rust, headless approach
based on a Dagre-compatible Rust layout library (`dugong`) plus a pluggable text measurement
interface; see ADR 0043.

## Consequences

- This document is retained for historical context and comparison.
