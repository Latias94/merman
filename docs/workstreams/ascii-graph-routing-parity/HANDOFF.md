# ASCII Graph Routing Parity - Handoff

Status: Active
Last updated: 2026-05-29

## Current State

The previous ASCII compatibility expansion is committed as
`da528d3d feat: expand ascii flowchart compatibility`.

AGR-010 is implemented and verified:

- `graph/model.rs` owns graph-only data types and test builders.
- `graph/adapter.rs` owns `FlowchartV2Model` conversion and validation.
- `graph/mod.rs` keeps rendering/layout behavior unchanged.

## Next Task

Run AGR-020:

- Add `graph_fixture` integration tests around copied `mermaid-ascii` graph fixtures.
- Keep an explicit allowlist of fixtures that currently match.
- Record unsupported fixtures as named gaps instead of implying full parity.
- Verify with `cargo nextest run -p merman-ascii graph_fixture` and flowchart gates.

## Constraints

- Do not add a second Mermaid parser.
- Do not use SVG layout as ASCII source of truth.
- Commit only verified bounded tasks.
- Stage only files touched for the current task.

## Target Window

Aim to stop, close, or hand off before 2026-05-29 09:00 +08:00.
