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
| 2026-05-30 | AFDT-020 | Added parser-backed BT/RL direction tests in `flowchart_model.rs`. | Both tests reproduce the current unsupported-direction diagnostic before implementation. |

## Verification Log

| Date | Task | Command | Scope | Result | Proves |
| --- | --- | --- | --- | --- | --- |
| 2026-05-30 | AFDT-010 | `git diff --check -- docs/workstreams/ascii-flowchart-direction-transform` | Workstream opening docs | PASS | Opening docs have no whitespace errors. |
| 2026-05-30 | AFDT-020 | `cargo nextest run -p merman-ascii flowchart_parser_bt_root_direction_renders_with_vertical_flip` | BT parser-backed direction contract | RED | Current behavior rejects BT with `non-LR/TD graph directions`. |
| 2026-05-30 | AFDT-020 | `cargo nextest run -p merman-ascii flowchart_parser_rl_root_direction_renders_with_horizontal_mirror` | RL parser-backed direction contract | RED | Current behavior rejects RL with `non-LR/TD graph directions`. |
| 2026-05-30 | AFDT-020 | `cargo fmt -p merman-ascii -- --check` | `merman-ascii` formatting | PASS | New flowchart direction tests are rustfmt-clean. |
| 2026-05-30 | AFDT-020 | `git diff --check -- crates/merman-ascii/tests/flowchart_model.rs docs/workstreams/ascii-flowchart-direction-transform` | AFDT-020 scoped diff | PASS | Test and workstream updates have no whitespace errors. |
