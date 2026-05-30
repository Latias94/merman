# ASCII Flowchart Multiline Subgraph Labels - Evidence And Gates

Status: Closed
Last updated: 2026-05-30

## Current Behavior

Multiline flowchart subgraph titles render as centered title rows for parser-backed Mermaid break
syntax and direct-model newline titles. Existing single-line subgraph fixture output remains stable.

## Gate Set

```bash
cargo nextest run -p merman-ascii flowchart
cargo nextest run -p merman-ascii graph_fixture
cargo nextest run -p merman-ascii
cargo fmt --all --check
git diff --check
```

## Evidence Log

| Date | Task | Evidence | Result |
| --- | --- | --- | --- |
| 2026-05-30 | AFMS-010 | Opened the multiline subgraph-label lane from the flowchart support matrix gap. | Scope is limited to subgraph title line breaks in `merman-ascii`. |
| 2026-05-30 | AFMS-020 | Added parser-backed and direct-model multiline subgraph title tests. | Tests fail against current raw `<br>` rendering and newline-title unsupported diagnostic. |
| 2026-05-30 | AFMS-030 | Implemented multiline subgraph title layout and drawing with `GraphLabel`. | Parser-backed and direct-model title tests pass; existing subgraph fixtures remain stable. |
| 2026-05-30 | AFMS-040 | Updated support docs and closed the workstream. | Explicit line-break subgraph titles are documented as supported; automatic wrapping remains deferred. |

## Verification Log

| Date | Task | Command | Scope | Result | Proves |
| --- | --- | --- | --- | --- | --- |
| 2026-05-30 | AFMS-010 | `git diff --check -- docs/workstreams/ascii-flowchart-multiline-subgraph-labels` | Workstream opening docs | PASS | Opening docs have no whitespace errors. |
| 2026-05-30 | AFMS-020 | `cargo nextest run -p merman-ascii flowchart_parser_multiline_subgraph_title_renders_centered_rows` | Parser-backed multiline title contract | RED | Current renderer emits raw `Line<br>Two` as one subgraph title row. |
| 2026-05-30 | AFMS-020 | `cargo nextest run -p merman-ascii render_flowchart_renders_model_multiline_subgraph_titles` | Direct-model newline title contract | RED | Current adapter rejects newline subgraph titles with `multiline subgraph labels`. |
| 2026-05-30 | AFMS-030 | `cargo nextest run -p merman-ascii flowchart_parser_multiline_subgraph_title_renders_centered_rows render_flowchart_renders_model_multiline_subgraph_titles` | Multiline title implementation contracts | PASS | Parser-backed `<br>` and direct-model newline titles render as centered rows. |
| 2026-05-30 | AFMS-030 | `cargo nextest run -p merman-ascii flowchart` | Flowchart parser/model/rendering tests | PASS | Multiline subgraph title support does not regress flowchart behavior. |
| 2026-05-30 | AFMS-030 | `cargo nextest run -p merman-ascii graph_fixture` | Copied graph fixture parity | PASS | Existing single-line and nested subgraph fixture output remains stable. |
| 2026-05-30 | AFMS-040 | `cargo nextest run -p merman-ascii` | Full `merman-ascii` regression suite | PASS | Final lane closeout still passes all `merman-ascii` tests. |
| 2026-05-30 | AFMS-040 | `cargo fmt --all --check` | Workspace formatting gate | PASS | Workspace formatting is clean. |
| 2026-05-30 | AFMS-040 | `git diff --check` | Whitespace and patch hygiene gate | PASS | No whitespace or patch-format errors remain. |
