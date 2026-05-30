# ASCII Flowchart Multiline Subgraph Labels

Status: Closed
Last updated: 2026-05-30

## Closeout

Closed on 2026-05-30. Multiline flowchart subgraph titles render through parser-backed and
direct-model ASCII tests, support docs describe the shipped explicit line-break behavior, and
automatic browser-style title wrapping remains deferred.

## Why This Lane Exists

`merman-ascii` already supports titled flowchart subgraph boxes, and `GraphLabel` already knows how
to normalize escaped newlines and HTML `<br>` breaks for node labels. Subgraph titles still have a
stricter boundary: hand-built titles containing real newlines are rejected, and parser-backed
titles using `<br>` are not rendered as multiple centered title rows.

This is the next small `beautiful-mermaid` delta after root-direction transforms: the title text is
already represented in the typed model, but group title layout and drawing need to reserve and paint
multiple rows honestly.

## Relevant Authority

- `docs/adr/0065-ascii-output-boundary.md`
- `docs/adr/0014-upstream-parity-policy.md`
- `crates/merman-ascii/FLOWCHART_SUPPORT.md`
- `docs/workstreams/ascii-flowchart-direction-transform/`

## Problem

Flowchart subgraph title rendering treats the title as a single raw string. That keeps simple
subgraphs stable, but it means multiline title syntax cannot become readable terminal output even
though the shared graph label machinery already supports the needed line splitting.

## Target State

- Parser-backed flowchart tests cover subgraph titles with Mermaid-supported line break syntax.
- Direct model coverage proves real newline titles render instead of hitting the old unsupported
  diagnostic.
- Group layout reserves enough vertical title space for all title rows.
- Group drawing centers each title line and keeps existing single-line subgraph output stable.
- Support docs move multiline subgraph labels from unsupported/deferred to supported subset.

## In Scope

- `merman-ascii` flowchart subgraph title layout and drawing.
- Parser-backed and direct-model tests through public rendering surfaces.
- Support matrix and workstream evidence updates.

## Out Of Scope

- `FlowSubgraph.dir` direction overrides.
- ANSI/HTML color roles, `classDef`, `class`, and inline styles.
- State diagram graph rendering.
- New uncommon flowchart shapes.
- Browser/SVG parity for wrapping long subgraph titles.

## Architecture Direction

Reuse `GraphLabel` for subgraph titles instead of introducing a second label splitting path.
Subgraph title sizing should be expressed in `layout.rs`, while title painting remains in
`draw.rs`. The adapter should no longer reject real newline subgraph titles, but it should keep
rejecting invalid subgraph member node ids containing newlines.

## Closeout Condition

This lane can close when multiline subgraph titles render through public tests, existing subgraph
fixtures remain stable, package gates pass, and support docs describe the shipped boundary.
