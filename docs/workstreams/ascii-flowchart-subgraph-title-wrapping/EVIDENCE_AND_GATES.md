# ASCII Flowchart Subgraph Title Wrapping — Evidence And Gates

Status: Closed
Last updated: 2026-05-30

## Current Behavior

Long parser-backed flowchart subgraph titles wrap into multiple centered title rows inside the
current group box width. Explicit `<br>`, escaped newline, and model newline title breaks remain
hard breaks through the shared graph-label path.

## Smallest Current Repro

```bash
cargo nextest run -p merman-ascii flowchart_parser_long_subgraph_title_wraps_to_multiple_rows
```

## Gate Set

### Targeted Iteration Gate

```bash
cargo nextest run -p merman-ascii flowchart_parser_long_subgraph_title_wraps_to_multiple_rows
```

### Package Gate

```bash
cargo nextest run -p merman-ascii flowchart
```

### Broader Closeout Gate

```bash
cargo nextest run -p merman-ascii
cargo fmt --all --check
git diff --check
cargo clippy -p merman-ascii --all-targets -- -D warnings
```

## Evidence Anchors

- `docs/workstreams/ascii-flowchart-subgraph-title-wrapping/DESIGN.md`
- `docs/workstreams/ascii-flowchart-subgraph-title-wrapping/TODO.md`
- `docs/workstreams/ascii-flowchart-subgraph-title-wrapping/MILESTONES.md`
- `docs/workstreams/ascii-flowchart-multiline-subgraph-labels/HANDOFF.md`
- `crates/merman-ascii/FLOWCHART_SUPPORT.md`
- `crates/merman-ascii/README.md`
- `repo-ref/mermaid/packages/mermaid/src/rendering-util/createText.ts`
- `repo-ref/mermaid/packages/mermaid/src/utils.ts`
- `repo-ref/beautiful-mermaid/src/ascii/multiline-utils.ts`

## Evidence Log

| Date | Task | Evidence | Result |
| --- | --- | --- | --- |
| 2026-05-30 | AFSW-010 | Opened the long-title wrapping lane from the multiline-title follow-on. | Scope is limited to `merman-ascii` flowchart subgraph titles. |
| 2026-05-30 | AFSW-020 | Added a parser-backed long-title wrapping contract. | The test failed red against raw one-line title expansion, then passed after implementation. |
| 2026-05-30 | AFSW-030 | Implemented `GraphLabel::wrapped` and fed wrapped titles through group layout/drawing. | Long titles wrap without expanding the group box; explicit multiline titles stay covered. |
| 2026-05-30 | AFSW-040 | Updated support docs and closed the workstream. | Wrapped subgraph titles are documented as shipped behavior. |

## Verification Log

| Date | Task | Command | Scope | Result | Proves |
| --- | --- | --- | --- | --- | --- |
| 2026-05-30 | AFSW-020 | `cargo nextest run -p merman-ascii flowchart_parser_long_subgraph_title_wraps_to_multiple_rows` | Parser-backed long-title contract | RED | Current renderer widened the group title to `Wrap this title nicely` on one raw row. |
| 2026-05-30 | AFSW-030 | `cargo nextest run -p merman-ascii flowchart_parser_long_subgraph_title_wraps_to_multiple_rows` | Parser-backed long-title contract | PASS | The same title wraps into multiple centered group-title rows. |
| 2026-05-30 | AFSW-030 | `cargo nextest run -p merman-ascii flowchart` | Flowchart parser/model/rendering tests | PASS, 32 tests | Wrapped titles do not regress existing flowchart behavior or explicit multiline titles. |
| 2026-05-30 | AFSW-030 | `cargo nextest run -p merman-ascii graph_fixture` | Copied graph fixture parity | PASS, 2 tests | Existing graph fixture inventory and allowlist stay stable. |
| 2026-05-30 | AFSW-040 | `cargo nextest run -p merman-ascii` | Full `merman-ascii` regression suite | PASS, 127 tests | Final lane closeout passes all `merman-ascii` tests. |
| 2026-05-30 | AFSW-040 | `cargo fmt --all --check` | Workspace formatting gate | PASS | Workspace formatting is clean. |
| 2026-05-30 | AFSW-040 | `git diff --check` | Whitespace and patch hygiene gate | PASS | No whitespace or patch-format errors remain. |
| 2026-05-30 | AFSW-040 | `cargo clippy -p merman-ascii --all-targets -- -D warnings` | `merman-ascii` lint gate | PASS | The implementation has no clippy warnings under the crate target set. |
