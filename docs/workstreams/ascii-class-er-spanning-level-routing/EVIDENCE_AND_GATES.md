# ASCII Class ER Spanning Level Routing - Evidence And Gates

Status: Active
Last updated: 2026-05-30

## Smallest Current Repro

Current spanning-level relationships reject with public diagnostics:

- `class relationships spanning multiple layout levels`
- `ER relationships spanning multiple layout levels`

The implementation slice should render a simple three-node transitive shape with a side lane for
the skipped relationship.

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
| 2026-05-30 | ACESLR-010 | Opened follow-on lane from `ascii-class-er-mixed-parallel-routing` closeout. | Scope is limited to non-cyclic spanning-level relationships. |

## Verification Log

| Date | Task | Command | Scope | Result | Proves |
| --- | --- | --- | --- | --- | --- |
| 2026-05-30 | ACESLR-010 | `git diff --check -- docs/workstreams/ascii-class-er-spanning-level-routing` | Workstream opening docs | PASS | Opening docs have no whitespace errors. |
