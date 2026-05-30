# ASCII Class ER Component Layout - Evidence And Gates

Status: Active
Last updated: 2026-05-30

## Smallest Current Repro

Current unrelated class/entity layouts are public unsupported diagnostics:

- `class_parser_relationship_layouts_with_unrelated_classes_are_explicitly_unsupported`
- `er_parser_relationship_layouts_with_unrelated_entities_are_explicitly_unsupported`

The first implementation slice should render a related component plus a standalone node while
preserving denser topology diagnostics inside each component.

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
| 2026-05-30 | ACECL-010 | Opened follow-on lane from `ascii-class-er-topology-routing` closeout. | Scope is limited to disconnected component layout. |

## Verification Log

| Date | Task | Command | Scope | Result | Proves |
| --- | --- | --- | --- | --- | --- |
| 2026-05-30 | ACECL-010 | `git diff --check -- docs/workstreams/ascii-class-er-component-layout` | Workstream opening docs | PASS | Opening docs have no whitespace errors. |
