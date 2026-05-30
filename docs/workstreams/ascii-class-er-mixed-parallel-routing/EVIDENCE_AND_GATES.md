# ASCII Class ER Mixed Parallel Routing - Evidence And Gates

Status: Active
Last updated: 2026-05-30

## Smallest Current Repro

Current mixed-parallel components still reject with public parallel diagnostics:

- `parallel class relationship layouts`
- `parallel ER relationship layouts`

The implementation slice should render a simple star-like component where one endpoint pair has two
parallel relationships and another endpoint pair has one ordinary relationship.

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
| 2026-05-30 | ACEMPR-010 | Opened follow-on lane from `ascii-class-er-parallel-routing` closeout. | Scope is limited to mixed-parallel relationship components. |

## Verification Log

| Date | Task | Command | Scope | Result | Proves |
| --- | --- | --- | --- | --- | --- |
| 2026-05-30 | ACEMPR-010 | `git diff --check -- docs/workstreams/ascii-class-er-mixed-parallel-routing` | Workstream opening docs | PASS | Opening docs have no whitespace errors. |
