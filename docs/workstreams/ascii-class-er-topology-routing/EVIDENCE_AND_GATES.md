# ASCII Class ER Topology Routing - Evidence And Gates

Status: Active
Last updated: 2026-05-30

## Smallest Current Repro

Current crossing layouts are public unsupported diagnostics:

- `class_parser_crossing_relationship_layouts_are_explicitly_unsupported`
- `er_parser_crossing_relationship_layouts_are_explicitly_unsupported`

The first implementation slice should replace those diagnostics with readable ASCII output for the
same two-by-two adjacent-layer fixtures while preserving unrelated/cyclic/parallel/spanning
diagnostics.

## Gate Set

### Focused Iteration

```bash
cargo nextest run -p merman-ascii class
cargo nextest run -p merman-ascii er
```

### Package And Hygiene

```bash
cargo nextest run -p merman-ascii
cargo clippy -p merman-ascii --all-targets -- -D warnings
cargo fmt --all --check
git diff --check
```

## Evidence Anchors

- `docs/workstreams/ascii-class-er-topology-routing/DESIGN.md`
- `docs/workstreams/ascii-class-er-topology-routing/TODO.md`
- `docs/workstreams/ascii-class-er-topology-routing/MILESTONES.md`
- `docs/workstreams/ascii-class-er-layered-planner/`
- `crates/merman-ascii/src/relation_graph.rs`
- `crates/merman-ascii/src/class/render.rs`
- `crates/merman-ascii/src/er/render.rs`
- `crates/merman-ascii/tests/class_model.rs`
- `crates/merman-ascii/tests/er_model.rs`

## Evidence Log

| Date | Task | Evidence | Result |
| --- | --- | --- | --- |
| 2026-05-30 | ACETR-010 | Opened follow-on lane from `ascii-class-er-layered-planner` closeout. | Scope is limited to crossing-first class/ER topology routing. |

## Verification Log

| Date | Task | Command | Scope | Result | Proves |
| --- | --- | --- | --- | --- | --- |
| 2026-05-30 | ACETR-010 | `git diff --check -- docs/workstreams/ascii-class-er-topology-routing` | Workstream opening docs | PASS | Opening docs have no whitespace errors. |
