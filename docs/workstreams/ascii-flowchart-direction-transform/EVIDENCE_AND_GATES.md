# ASCII Flowchart Direction Transform - Evidence And Gates

Status: Closed
Last updated: 2026-05-30

## Current Behavior

BT and RL root directions now render through the public ASCII flowchart surface. BT reuses the TD
layout and vertically flips the output; RL reuses the LR layout and horizontally mirrors the output.
LR and TD golden outputs remain stable.

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
| 2026-05-30 | AFDT-030 | Implemented render-layer BT/RL transforms in `crates/merman-ascii/src/graph`. | BT/RL pass through the public flowchart renderer; mirrored node and edge labels stay readable. |
| 2026-05-30 | AFDT-040 | Updated public support docs and closed the workstream. | BT/RL root directions are documented as shipped; remaining work is deferred to narrower follow-ons. |

## Verification Log

| Date | Task | Command | Scope | Result | Proves |
| --- | --- | --- | --- | --- | --- |
| 2026-05-30 | AFDT-010 | `git diff --check -- docs/workstreams/ascii-flowchart-direction-transform` | Workstream opening docs | PASS | Opening docs have no whitespace errors. |
| 2026-05-30 | AFDT-020 | `cargo nextest run -p merman-ascii flowchart_parser_bt_root_direction_renders_with_vertical_flip` | BT parser-backed direction contract | RED | Pre-implementation behavior rejected BT with `non-LR/TD graph directions`. |
| 2026-05-30 | AFDT-020 | `cargo nextest run -p merman-ascii flowchart_parser_rl_root_direction_renders_with_horizontal_mirror` | RL parser-backed direction contract | RED | Pre-implementation behavior rejected RL with `non-LR/TD graph directions`. |
| 2026-05-30 | AFDT-020 | `cargo fmt -p merman-ascii -- --check` | `merman-ascii` formatting | PASS | New flowchart direction tests are rustfmt-clean. |
| 2026-05-30 | AFDT-020 | `git diff --check -- crates/merman-ascii/tests/flowchart_model.rs docs/workstreams/ascii-flowchart-direction-transform` | AFDT-020 scoped diff | PASS | Test and workstream updates have no whitespace errors. |
| 2026-05-30 | AFDT-030 | `cargo nextest run -p merman-ascii flowchart_parser_rl_multi_character_node_labels_stay_readable flowchart_parser_rl_edge_labels_stay_readable` | RL mirrored text contracts | PASS | Horizontal mirroring does not reverse node labels or edge labels. |
| 2026-05-30 | AFDT-030 | `cargo nextest run -p merman-ascii flowchart_parser_rl_chain_mirrors_unicode_connectors` | RL Unicode mirror contract | PASS | Unicode connector and arrowhead characters mirror correctly. |
| 2026-05-30 | AFDT-030 | `cargo nextest run -p merman-ascii flowchart` | Flowchart parser/model/rendering tests | PASS | BT/RL render green and LR/TD flowchart golden outputs stay stable. |
| 2026-05-30 | AFDT-030 | `cargo nextest run -p merman-ascii` | Full `merman-ascii` package | PASS | ASCII package behavior remains green after direction transforms. |
| 2026-05-30 | AFDT-030 | `cargo clippy -p merman-ascii --all-targets -- -D warnings` | `merman-ascii` lint gate | PASS | Direction transform implementation is clippy-clean. |
| 2026-05-30 | AFDT-030 | `cargo fmt --all --check` | Workspace formatting | PASS | Workspace formatting remains rustfmt-clean. |
| 2026-05-30 | AFDT-030 | `git diff --check` | Current worktree diff | PASS | Implementation and workstream updates have no whitespace errors. |
| 2026-05-30 | AFDT-040 | `cargo nextest run -p merman-ascii` | Final `merman-ascii` package gate | PASS | Package tests remain green after support-doc closeout. |
| 2026-05-30 | AFDT-040 | `cargo clippy -p merman-ascii --all-targets -- -D warnings` | Final `merman-ascii` lint gate | PASS | Package lints remain green at closeout. |
| 2026-05-30 | AFDT-040 | `cargo fmt --all --check` | Final workspace formatting gate | PASS | Workspace formatting remains rustfmt-clean at closeout. |
| 2026-05-30 | AFDT-040 | `git diff --check` | Final closeout diff | PASS | Closeout docs have no whitespace errors. |
