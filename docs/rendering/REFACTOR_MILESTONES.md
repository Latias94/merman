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
  - `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3 --flowchart-text-measurer vendored`
  - `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode structure --dom-decimals 3 --flowchart-text-measurer vendored`
  - `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --flowchart-text-measurer vendored`

## Milestones

### R0: Baseline documentation (done when merged)

Deliverables:

- A design doc describing the target architecture.
- A TODO doc listing incremental steps.
- A milestone doc describing what “done” means.

Exit criteria:

- No code changes required.

### R1: Shared root viewport + `<svg>` root writer (starter)

Deliverables:

- A shared helper for root viewport override application (viewBox + max-width).
- A shared root `<svg>` writer adopted in **one** diagram end-to-end (recommended: ER).

Exit criteria:

- All gates green.
- The new helper is used in at least one diagram with unchanged DOM output.

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
  `packet`, `pie`, `xychart`, `block`, `error`, `treemap`, `info`, `quadrantchart`, `sankey`, `radar`, `c4`.

### R3: Diagram render modules normalized

Deliverables:

- Flowchart and Class are split into consistent submodules (root/css/defs/render).
- A consistent per-diagram public entry point naming pattern.

Exit criteria:

- Reduced file sizes for the largest renderers (`flowchart.rs`, `class.rs`) without behavior change.

### R4: Tooling automation for overrides

Deliverables:

- A stable `xtask` command that can update root viewport overrides from a compare report.
- Inventory/reporting for stale overrides.

Exit criteria:

- Updating overrides is “one command” and can be done without manual copy/paste.
