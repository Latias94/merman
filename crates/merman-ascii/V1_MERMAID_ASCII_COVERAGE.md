# V1 Mermaid-Ascii Coverage Contract

Status: active v1 release gate
Last updated: 2026-06-02

This document defines the first `merman-ascii` release boundary for compatibility with the
MIT-licensed `AlexanderGrooff/mermaid-ascii` reference implementation.

## Reference Scope

The v1 gate is the tracked fixture corpus copied from:

- Source: `repo-ref/mermaid-ascii`
- Source commit: `6fffb8e2714acab2c4cb41c78894fabbc62cee56`
- Source path: `cmd/testdata`
- Tracked copy: `crates/merman-ascii/tests/testdata/mermaid-ascii`

`repo-ref/` is a research checkout and is not required by CI or downstream users. The tracked copy
is the executable source of truth for the v1 coverage contract.

## Coverage Matrix

| Reference fixture group | Diagram scope | Current v1 status | Gate |
| --- | --- | --- | --- |
| `ascii` | `graph` / `flowchart` LR, TD, TB fixtures with ASCII characters | 52 / 52 exact output matches | `cargo nextest run -p merman-ascii graph_fixture` |
| `extended-chars` | `graph` / `flowchart` LR, TD, TB fixtures with Unicode box drawing characters | 23 / 23 exact output matches | `cargo nextest run -p merman-ascii graph_fixture` |
| `sequence` | `sequenceDiagram` fixtures with Unicode box drawing characters | 12 / 12 normalized exact output matches | `cargo nextest run -p merman-ascii sequence_golden` |
| `sequence-ascii` | `sequenceDiagram` fixtures with ASCII characters | 5 / 5 normalized exact output matches | `cargo nextest run -p merman-ascii sequence_golden` |

Summary:

- Graph/flowchart copied fixture parity: 75 / 75.
- Sequence copied fixture parity: 17 / 17.
- Named copied fixture gaps: none.

## V1 Gate

Run the focused v1 compatibility gate with:

```bash
cargo nextest run -p merman-ascii fixture_inventory graph_fixture sequence_golden
```

Before release, also run the package gate:

```bash
cargo nextest run -p merman-ascii
```

`fixture_inventory` pins the copied fixture counts and source provenance. `graph_fixture` and
`sequence_golden` prove exact copied-fixture compatibility for the supported reference scope.

## Non-Goals For V1

V1 does not mean full Mermaid parity. It means the terminal renderer covers the practical
`mermaid-ascii` graph/flowchart and sequence corpus without relying on manual inspection.

The following families are product extensions beyond the `mermaid-ascii` v1 gate:

- `classDiagram`
- `erDiagram`
- `stateDiagram`
- `xychart`
- any Mermaid family not rendered by `repo-ref/mermaid-ascii`

These families should keep their own parser-backed tests and support matrices, but they are not
required to block the first `mermaid-ascii`-coverage release unless explicitly promoted into the
release gate.

The copied upstream corpus remains the exact v1 oracle. Self-authored semantic fixtures may exist
for complex class, ER, state, or xychart cases, but they are not part of the copied release gate
and do not change the v1 inventory contract.

`repo-ref/beautiful-mermaid` is useful as a design reference for richer terminal output and broader
diagram families. It is not a byte-for-byte output oracle for this v1 gate.
