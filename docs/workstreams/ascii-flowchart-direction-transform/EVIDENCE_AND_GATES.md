# ASCII Flowchart Direction Transform - Evidence And Gates

Status: Active
Last updated: 2026-05-30

## Smallest Current Repro

Current BT/RL flowchart directions reject with public diagnostics:

- `non-LR/TD graph directions`

The implementation slice should render BT and RL through the public ASCII flowchart surface while
preserving LR/TD output.

## Gate Set

```bash
cargo nextest run -p merman-ascii flowchart
cargo clippy -p merman-ascii --all-targets -- -D warnings
cargo fmt --all --check
git diff --check
```

## Evidence Log

| Date | Task | Evidence | Result |
| --- | --- | --- | --- |
| 2026-05-30 | AFDT-010 | Opened the flowchart BT/RL direction-transform lane from the support-matrix gap. | Scope is limited to root-direction transforms only. |

## Verification Log

| Date | Task | Command | Scope | Result | Proves |
| --- | --- | --- | --- | --- | --- |
| 2026-05-30 | AFDT-010 | `git diff --check -- docs/workstreams/ascii-flowchart-direction-transform` | Workstream opening docs | PASS | Opening docs have no whitespace errors. |
