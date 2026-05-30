# ASCII Flowchart Multiline Subgraph Labels - Milestones

Status: Closed
Last updated: 2026-05-30

## Closeout Summary

All milestones are complete. Multiline subgraph titles now reuse `GraphLabel` for layout and
drawing. Subgraph direction overrides, style/color roles, state diagram graph rendering, uncommon
shapes, and automatic long-title wrapping remain deferred follow-ons.

## M0 - Scope And Evidence Freeze

Exit criteria:

- Problem and target state are explicit.
- Non-goals are explicit.
- Relevant ADRs/docs/workstreams are linked.

Primary evidence:

- `docs/workstreams/ascii-flowchart-multiline-subgraph-labels/DESIGN.md`
- `docs/workstreams/ascii-flowchart-multiline-subgraph-labels/TODO.md`

## M1 - Contract Tests

Exit criteria:

- Parser-backed multiline subgraph title behavior is specified.
- Direct-model real newline title behavior is specified.
- Tests fail before implementation for the current limitation.

Primary gates:

- Targeted `cargo nextest run -p merman-ascii` filters for new tests.

## M2 - Layout And Drawing

Exit criteria:

- Multiline subgraph titles render as centered title rows.
- Existing single-line subgraph fixtures remain stable.
- The implementation reuses `GraphLabel`.

Primary gates:

- `cargo nextest run -p merman-ascii flowchart`
- `cargo nextest run -p merman-ascii graph_fixture`

## M3 - Docs And Closeout

Exit criteria:

- Support docs describe shipped multiline subgraph title behavior.
- Final package and formatting gates pass.
- Remaining title/layout work is split or deferred.

Primary gates:

- `cargo nextest run -p merman-ascii`
- `cargo fmt --all --check`
- `git diff --check`
