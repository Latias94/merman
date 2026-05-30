# ASCII Class ER Parallel Routing - Evidence And Gates

Status: Active
Last updated: 2026-05-30

## Smallest Current Repro

Current same-endpoint parallel layouts are public unsupported diagnostics:

- `parallel class relationship layouts`
- `parallel ER relationship layouts`

The first implementation slice should render multiple relationships between the same two endpoints
as adjacent terminal lanes while preserving each relationship's text semantics.

## Gate Set

```bash
cargo nextest run -p merman-ascii class
cargo nextest run -p merman-ascii er
cargo nextest run -p merman-ascii
cargo clippy -p merman-ascii --all-targets -- -D warnings
cargo fmt --all --check
git diff --check
```

## Evidence Log

| Date | Task | Evidence | Result |
| --- | --- | --- | --- |
| 2026-05-30 | ACEPR-010 | Opened follow-on lane from class/ER topology and component closeouts. | Scope is limited to same-endpoint parallel relationship routing. |

## Verification Log

| Date | Task | Command | Scope | Result | Proves |
| --- | --- | --- | --- | --- | --- |
| 2026-05-30 | ACEPR-010 | `git diff --check -- docs/workstreams/ascii-class-er-parallel-routing` | Workstream opening docs | PASS | Opening docs have no whitespace errors. |
