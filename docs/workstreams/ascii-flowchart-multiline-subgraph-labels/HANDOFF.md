# ASCII Flowchart Multiline Subgraph Labels - Handoff

Status: Active
Last updated: 2026-05-30

## Current State

Parser-backed and direct-model multiline subgraph title contracts are now captured and red.
Existing code rejects direct-model subgraph titles with real newlines and renders parser-preserved
break syntax as a raw title string instead of multiple centered title rows.

## Active Task

- Task ID: AFMS-030
- Owner: unassigned
- Files:
  - `crates/merman-ascii/src/graph`
  - `crates/merman-ascii/src/lib.rs`
- Validation: `cargo nextest run -p merman-ascii flowchart`; `cargo nextest run -p merman-ascii graph_fixture`
- Status: READY
- Review: Reuse `GraphLabel`, keep parser/core unchanged, and preserve existing single-line
  subgraph fixture output.
- Evidence: `EVIDENCE_AND_GATES.md`

## Decisions Since Opening

- Scope is limited to flowchart subgraph titles in `merman-ascii`.
- `GraphLabel` is the intended shared line-break model.
- Subgraph direction overrides and style/color roles remain separate follow-ons.
- AFMS-020 added `flowchart_parser_multiline_subgraph_title_renders_centered_rows` and
  `render_flowchart_renders_model_multiline_subgraph_titles`.

## Blockers

- None.

## Next Recommended Action

- Start AFMS-030 by making the multiline subgraph title tests green without changing parser/core.
