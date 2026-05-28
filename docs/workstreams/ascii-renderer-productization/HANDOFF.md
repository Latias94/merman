# ASCII Renderer Productization - Handoff

Status: Active
Last updated: 2026-05-28

## Current State

The ASCII renderer productization lane is active. ARP-020 through ARP-060 are complete:
`merman-ascii` has its crate/API foundation, tracked `mermaid-ascii` attribution, copied golden
fixtures, text/canvas primitives, the first graph rendering slice, parser/model-level flowchart
tests, a documented flowchart support matrix, and the first sequence rendering slice. Basic
flowcharts with boxed nodes and direct left-to-right or top-down edges can render through
`render_flowchart`. Basic sequence diagrams with participant boxes, lifelines, solid/dotted
messages, reverse messages, self messages, labels, and visible autonumber can render through
`render_sequence` or `render_model`.

## Active Task

- Task ID: ARP-070
- Owner: unassigned
- Files: `crates/merman-ascii`, `crates/merman`
- Validation:
  - `cargo check -p merman --features ascii`
  - `cargo nextest run -p merman-ascii`
  - `cargo nextest run -p merman --features ascii`
- Status: READY
- Review: Run `review-workstream` before accepting completion.
- Evidence: future public API tests and README examples.

## Decisions Since Last Update

- ASCII output should live in a new `merman-ascii` crate.
- The crate should consume `merman-core` typed models instead of parsing Mermaid text.
- `repo-ref/mermaid-ascii` is an algorithm and fixture reference, not an authoritative dependency.
- Third-party MIT license text and source commit provenance must be tracked before derived code or
  copied fixtures ship.
- Flowchart should be the first substantial rendering slice, followed by sequence.
- ARP-020/030 established a temporary explicit unsupported-feature boundary so the public API can
  compile before graph and sequence algorithms are ported.
- ARP-040 introduced a minimal `FlowchartV2Model` bridge only to route simple public
  `render_flowchart` calls through the graph primitives. ARP-050 should harden that adapter,
  document the supported feature matrix, and add model-level tests.
- ARP-050 hardened the parser/model-level flowchart path and documented
  `crates/merman-ascii/FLOWCHART_SUPPORT.md`.
- ARP-060 ported the initial sequence layout/drawing algorithm from the copied
  `mermaid-ascii` fixture subset, documented `crates/merman-ascii/SEQUENCE_SUPPORT.md`, and added
  explicit unsupported-feature diagnostics for non-basic sequence semantics.

## Blockers

- None for ARP-070.

## Next Recommended Action

- Execute ARP-070 with `run-workstream-task`: expose the ASCII renderer through a stable top-level
  library feature, add API examples/tests, and keep CLI integration split unless the public API
  surface is already stable.
