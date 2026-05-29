# Sequence Fixture Gaps

Status: No copied upstream sequence fixture gaps
Last updated: 2026-05-29

## Copied Upstream Fixtures

`merman-ascii` currently matches every copied `repo-ref/mermaid-ascii` sequence fixture under the
same normalized-whitespace comparison used by the upstream Go tests.

| Fixture group | Exact matches | Notes |
| --- | ---: | --- |
| `sequence` | 12 / 12 | Unicode sequence output. |
| `sequence-ascii` | 5 / 5 | ASCII sequence output. |

## Upstream Algorithm Boundary

The copied upstream sequence renderer is intentionally small. It covers:

- participant declarations and aliases
- implicit participants from messages
- participant boxes and lifelines
- `->>` solid messages
- `-->>` dotted messages
- self messages
- message labels
- autonumber
- ASCII and Unicode character sets

It does not cover activation boxes or loop/alt/opt/par blocks in the upstream README checklist.

## Merman Product Gaps

`merman-ascii` consumes `merman-core` typed sequence models, which are richer than the upstream
`mermaid-ascii` parser. Product gaps beyond copied fixture parity are tracked in
`crates/merman-ascii/SEQUENCE_SUPPORT.md` and the `ascii-sequence-parity` workstream.
