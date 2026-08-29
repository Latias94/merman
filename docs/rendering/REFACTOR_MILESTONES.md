# Rendering Refactor Milestones (Fearless, Parity-First)

This document tracks **structural** refactor milestones for the SVG parity renderers under
`crates/merman-render/src/svg/parity/*`.

Scope:

- “Fearless refactoring” here means **behavior-preserving** changes first, enabled by strong gates.
- Performance work is explicitly out of scope unless it is a direct consequence of simplification.

## Gates (must stay green)

- `cargo fmt --check`
- `cargo nextest run`
- SVG DOM gates:
  - `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3 --diagnostic-browser-text-layout`
  - `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode structure --dom-decimals 3 --diagnostic-browser-text-layout`
  - `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --diagnostic-browser-text-layout`

## Milestones

### R0: Baseline documentation (done when merged)

Deliverables:

- A design doc describing the target architecture.
- A TODO doc listing incremental steps.
- A milestone doc describing what “done” means.

Exit criteria:

- No code changes required.

### R1: Shared Root Viewport + `<svg>` ownership

Deliverables:

- One Root Viewport interface for computed family or emitted-content bounds, sizing, and root
  attributes.
- Shared root emission adopted by every built-in family.

Exit criteria:

- All gates green.
- Direct family root emission and production fixture pins are absent.

### R2: Root writer adopted across Stage B diagrams

Deliverables:

- A single shared implementation for:
  - opening root `<svg>`
  - aria attributes
  - `<title>` / `<desc>` emission
  - `<style>` wrapper
  - closing root `<svg>`

Exit criteria:

- All Stage B diagrams use the shared root writer.
- No fixture-id keyed behavior is introduced.

Status (rolling):

- Root writer adopted in: `er`, `requirement`, `journey`, `timeline`, `kanban`, `gitgraph`, `gantt`,
  `packet`, `pie`, `xychart`, `block`, `error`, `treemap`, `info`, `quadrantchart`, `sankey`, `radar`, `c4`, `mindmap`,
  `architecture`, `flowchart`, `class`, `sequence`, `state`.

### R3: Diagram render modules normalized

Deliverables:

- Flowchart and Class are split into consistent submodules (root/css/defs/render).
- A consistent per-diagram public entry point naming pattern.

Exit criteria:

- Reduced file sizes for the largest renderers (`flowchart.rs`, `class.rs`) without behavior change.

### R4: Generalized measurement-profile tooling

Deliverables:

- Browser/font residuals are measured as verification evidence; no `gen-font-metrics` command or
  generated font table is part of the production toolchain.
- An architecture guard that rejects fixture ids and complete-label answers in production data.

Exit criteria:

- Generated profiles validate against an independent corpus without adding a runtime fixture path.
