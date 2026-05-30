# ASCII Class ER Topology Routing - Evidence And Gates

Status: Closed
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
| 2026-05-30 | ACETR-020 | Added parser-backed class and ER crossing output tests. | Both tests failed red on the previous explicit unsupported diagnostics. |
| 2026-05-30 | ACETR-030 | Shared planner now reorders child layers by previous-layer parent order before crossing validation. | Adjacent-layer crossing class/ER relationships render every edge by producing a non-crossing layer order. |
| 2026-05-30 | ACETR-040 | Updated public support docs and ran final package, lint, formatting, and whitespace gates. | Lane closes with adjacent-layer crossing support shipped and remaining dense topology work deferred. |

## Verification Log

| Date | Task | Command | Scope | Result | Proves |
| --- | --- | --- | --- | --- | --- |
| 2026-05-30 | ACETR-010 | `git diff --check -- docs/workstreams/ascii-class-er-topology-routing` | Workstream opening docs | PASS | Opening docs have no whitespace errors. |
| 2026-05-30 | ACETR-020 | `cargo nextest run -p merman-ascii class_parser_crossing_relationship_layout_reorders_layer_to_render_each_edge` | Class crossing tracer | FAIL, expected red | Existing behavior rejected crossing class relationships. |
| 2026-05-30 | ACETR-020 | `cargo nextest run -p merman-ascii er_parser_crossing_relationship_layout_reorders_layer_to_render_each_edge` | ER crossing tracer | FAIL, expected red | Existing behavior rejected crossing ER relationships. |
| 2026-05-30 | ACETR-030 | `cargo nextest run -p merman-ascii class_parser_crossing_relationship_layout_reorders_layer_to_render_each_edge` | Class crossing tracer | PASS, 1 test | Class crossing relationships render by reordering the lower layer. |
| 2026-05-30 | ACETR-030 | `cargo nextest run -p merman-ascii er_parser_crossing_relationship_layout_reorders_layer_to_render_each_edge` | ER crossing tracer | PASS, 1 test | ER crossing relationships render by reordering the lower layer. |
| 2026-05-30 | ACETR-030 | `cargo nextest run -p merman-ascii class` | Focused class ASCII tests | PASS, 14 tests | Existing class behavior and unrelated diagnostics remain stable. |
| 2026-05-30 | ACETR-030 | `cargo nextest run -p merman-ascii er` | Focused ER/filter gate | PASS, 81 tests | Existing ER behavior and unrelated diagnostics remain stable. |
| 2026-05-30 | ACETR-030 | `cargo clippy -p merman-ascii --all-targets -- -D warnings` | merman-ascii lint gate | PASS | Planner crossing support is warning-free. |
| 2026-05-30 | ACETR-030 | `cargo fmt --all --check` | Workspace formatting check | PASS | Implementation formatting is stable. |
| 2026-05-30 | ACETR-030 | `git diff --check -- crates/merman-ascii/src/relation_graph.rs crates/merman-ascii/tests/class_model.rs crates/merman-ascii/tests/er_model.rs docs/workstreams/ascii-class-er-topology-routing` | Scoped whitespace hygiene | PASS | ACETR-030 files have no whitespace errors. |
| 2026-05-30 | ACETR-040 | `cargo nextest run -p merman-ascii` | Full ASCII package gate | PASS, 113 tests | Planner crossing support remains compatible with the full terminal renderer package. |
| 2026-05-30 | ACETR-040 | `cargo clippy -p merman-ascii --all-targets -- -D warnings` | merman-ascii lint gate | PASS | Final shared planner state is warning-free. |
| 2026-05-30 | ACETR-040 | `cargo fmt --all --check` | Workspace formatting check | PASS | Final lane state is formatted. |
| 2026-05-30 | ACETR-040 | `git diff --check` | Workspace whitespace hygiene | PASS | Final lane state has no whitespace errors. |

## Closeout Review

No blocking findings remain for the crossing-first target state. The shipped support is intentionally
limited to adjacent-layer crossing layouts resolvable by layer reordering. Dense, parallel, cyclic,
spanning-level, unrelated-component, and unresolved crossing topology routing should remain separate
workstreams.
