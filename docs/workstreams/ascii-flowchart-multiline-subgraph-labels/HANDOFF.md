# ASCII Flowchart Multiline Subgraph Labels - Handoff

Status: Closed
Last updated: 2026-05-30

## Current State

Multiline flowchart subgraph titles now render as centered title rows. Parser-backed `<br>` titles
and direct-model newline titles are green, existing subgraph fixtures remain stable, and support
docs describe the shipped explicit line-break behavior.

## Active Task

None. This workstream is closed.

## Decisions Since Opening

- Scope is limited to flowchart subgraph titles in `merman-ascii`.
- `GraphLabel` is the intended shared line-break model.
- Subgraph direction overrides and style/color roles remain separate follow-ons.
- AFMS-020 added `flowchart_parser_multiline_subgraph_title_renders_centered_rows` and
  `render_flowchart_renders_model_multiline_subgraph_titles`.
- AFMS-030 reused `GraphLabel` for subgraph title width/height and drawing.
- AFMS-040 moved multiline subgraph labels from unsupported/deferred to supported subset in the
  public support docs.

## Blockers

- None.

## Follow-Ons

- Automatic browser-style wrapping for long subgraph titles.
- Subgraph direction overrides from `FlowSubgraph.dir`.
- ANSI/HTML color roles and style interpretation.
