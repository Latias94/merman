# ASCII Flowchart Multiline Subgraph Labels - Evidence And Gates

Status: Active
Last updated: 2026-05-30

## Smallest Current Repro

The current renderer does not have a tested multiline subgraph title contract. The expected lane
proof is a parser-backed title using Mermaid-supported break syntax and a hand-built model title
containing a real newline.

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

## Verification Log

| Date | Task | Command | Scope | Result | Proves |
| --- | --- | --- | --- | --- | --- |
| 2026-05-30 | AFMS-010 | `git diff --check -- docs/workstreams/ascii-flowchart-multiline-subgraph-labels` | Workstream opening docs | PASS | Opening docs have no whitespace errors. |
| 2026-05-30 | AFMS-020 | `cargo nextest run -p merman-ascii flowchart_parser_multiline_subgraph_title_renders_centered_rows` | Parser-backed multiline title contract | RED | Current renderer emits raw `Line<br>Two` as one subgraph title row. |
| 2026-05-30 | AFMS-020 | `cargo nextest run -p merman-ascii render_flowchart_renders_model_multiline_subgraph_titles` | Direct-model newline title contract | RED | Current adapter rejects newline subgraph titles with `multiline subgraph labels`. |
