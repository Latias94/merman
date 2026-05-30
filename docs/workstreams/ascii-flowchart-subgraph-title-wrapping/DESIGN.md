# ASCII Flowchart Subgraph Title Wrapping

Status: Closed
Last updated: 2026-05-30

## Why This Lane Exists

`merman-ascii` now renders explicit multiline subgraph titles, but long single-line titles still
follow the raw one-line width path. That keeps some flowchart subgraph boxes too wide and leaves
the shipped ASCII output short of browser-style wrapping.

## Relevant Authority

- ADRs:
  - `docs/adr/0065-ascii-output-boundary.md`
  - `docs/adr/0014-upstream-parity-policy.md`
- Existing docs:
  - `crates/merman-ascii/FLOWCHART_SUPPORT.md`
  - `crates/merman-ascii/README.md`
  - `docs/workstreams/ascii-flowchart-multiline-subgraph-labels/HANDOFF.md`
- Related workstreams:
  - `docs/workstreams/ascii-flowchart-multiline-subgraph-labels`

## Problem

Long subgraph titles are measured as a single raw row. The layout either stretches the group to the
full title width or keeps the same height model as explicit line breaks, so the box does not
reflow the way browser-style Mermaid output does.

## Target State

Long subgraph titles automatically wrap within the available group width, the title height grows
accordingly, explicit breaks still act as hard breaks, and existing flowchart fixtures stay stable.

## In Scope

- flowchart subgraph title wrapping in `merman-ascii`
- shared graph label width and height handling
- parser-backed tests for wrapped titles and explicit-break stability
- support docs and README notes

## Out Of Scope

- node, edge, sequence, class, ER, or state text wrapping
- parser changes
- subgraph direction overrides
- color or style roles

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| `wrap_display_lines` in `crates/merman-ascii/src/text.rs` is the right terminal-width primitive. | High | It already handles word wrapping and long words by display width. | We would need a second text-wrapping helper. |
| Wrap width can be derived from the current group inner width before title expansion. | Medium | Existing group layout already computes a stable content box width. | The lane may need a cap or a separate title-width policy. |
| Explicit `<br>`/escaped-newline titles remain hard breaks and are still supported. | High | The multiline-title lane already shipped that behavior. | We would regress the closed multiline lane. |
| A shared `GraphLabel` wrapper is the right place to keep explicit breaks and auto-wrap together. | High | `GraphLabel` already owns line splitting and display width. | Title handling would become split across more modules. |

## Architecture Direction

Keep the behavior inside `graph::label`, `graph::layout`, and `graph::draw`. Avoid parser changes;
feed a wrapped label into layout and drawing. Layout must compute title height from the wrapped
lines and must not let the raw string expand the box before wrapping.

## Closeout Condition

This lane is closed because:

- long-title wrap tests pass,
- existing explicit-break subgraph tests stay green,
- support docs describe the shipped wrapping behavior,
- and follow-on work remains limited to separate layout/text capabilities.
