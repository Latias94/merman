# ADR 0042: Rendering Strategy (SVG) for Mermaid Parity

## Status

Proposed

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

Adopt a staged strategy:

1) Make **JS-backed embedded rendering** (Option B) the primary strategy for SVG parity.
2) Keep Rust-native rendering (Option A) as a long-term effort, potentially replacing parts of the
   JS-backed pipeline behind feature flags without changing the public API.

Implementation direction:

- Add a new workspace crate `merman-render` with a small, runtime-agnostic API surface:
  - input: original Mermaid text + parsed/effective config (or a config override)
  - output: SVG string + optional diagnostics (warnings, timing)
- Add `merman-render-js` as an optional backend that embeds a JS engine and runs Mermaid `@11.12.2`.
- Add renderer parity tests based on upstream SVG snapshots where available, or deterministic
  “structural SVG checks” (e.g. stable IDs, required nodes/edges) when upstream does not snapshot.

## Consequences

- `merman` can reach “complete Mermaid support” for SVG without waiting for Rust-native layout work.
- The workspace gains a well-defined boundary between parsing/semantic model and rendering.
- We must design and document sandbox/resource-limit behavior for the embedded JS runtime.
- Distribution requires bundling the pinned Mermaid JS artifacts and auditing licensing/compliance.

