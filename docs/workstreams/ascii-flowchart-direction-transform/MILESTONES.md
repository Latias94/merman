# ASCII Flowchart Direction Transform - Milestones

Status: Active
Last updated: 2026-05-30

## M0 - Scope And Evidence Freeze

Exit criteria:

- Problem and target state are explicit.
- Non-goals are explicit.
- Relevant ADRs/docs/workstreams are linked.
- First proof target is chosen.

Primary evidence:

- `docs/workstreams/ascii-flowchart-direction-transform/DESIGN.md`
- `docs/workstreams/ascii-flowchart-direction-transform/TODO.md`

## M1 - Direction Contract Tests

Exit criteria:

- BT and RL behavior are specified by parser-backed tests.
- Tests fail before implementation on the current unsupported diagnostic.

Primary gates:

- `cargo nextest run -p merman-ascii flowchart`

## M2 - Root Direction Transform

Exit criteria:

- BT and RL render through the public ASCII flowchart surface.
- LR and TD remain stable.
- The transform stays in the ASCII adapter layer.

Primary gates:

- `cargo nextest run -p merman-ascii flowchart`
- `cargo clippy -p merman-ascii --all-targets -- -D warnings`

## M3 - Docs And Closeout

Exit criteria:

- Support docs describe shipped BT/RL behavior.
- Final package and formatting gates pass.
- Remaining flowchart direction work is split or deferred.

Primary gates:

- `cargo nextest run -p merman-ascii`
- `cargo fmt --all --check`
- `git diff --check`
