# ASCII Renderer Compatibility Expansion - Handoff

Status: Complete
Last updated: 2026-05-28

## Current State

This workstream is complete. The previous `ascii-renderer-productization` lane introduced:

- `crates/merman-ascii`
- Top-level `merman` `ascii` feature
- CLI `--format ascii|unicode`
- Initial flowchart support for rectangular LR/TD graphs
- Initial sequence support for participants and basic messages

## Completed In This Lane

- ACE-010: compatibility policy and `FLOWCHART_SUPPORT.md` V1.1 plan.
- ACE-020: common flowchart edge labels, open/dotted edges, and length spacing.
- ACE-030: common flowchart node-shape approximations.
- ACE-040: simple titled flowchart subgraph boxes.
- ACE-050: README examples, changelog, and expanded CLI ASCII smoke coverage.
- ACE-060: closeout gates and completion evidence.

## Follow-Ons

- Complex nested subgraph routing and branch-heavy flowchart routing remain follow-ons.
- Less common flowchart shapes such as hexagon, lean/document variants, fork/join, icons, and
  images remain explicit unsupported cases.
- Sequence compatibility expansion remains a separate future lane.

## Important Constraints

- Do not add a second Mermaid parser.
- Keep ASCII output model-driven from `merman-core`.
- Preserve explicit unsupported-feature diagnostics where representation is not designed yet.
- Treat ASCII snapshots as product behavior.

## Suggested Commands

- `cargo nextest run -p merman-ascii flowchart`
- `cargo nextest run -p merman-ascii graph::`
- `cargo check -p merman-ascii`
- `cargo nextest run -p merman-cli --features ascii`
- `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
