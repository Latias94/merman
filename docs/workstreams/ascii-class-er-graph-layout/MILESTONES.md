# ASCII Class ER Graph Layout - Milestones

Status: Active
Last updated: 2026-05-30

## M0 - Lane Opening

Exit criteria:

- The lane has a narrow problem statement and does not reopen the closed reference-expansion lane.
- Follow-on candidates outside class/ER graph layout remain out of scope.

Primary evidence:

- `DESIGN.md`
- `TODO.md`
- `WORKSTREAM.json`

## M1 - Contract Tracers

Exit criteria:

- Current unsupported class and ER multi-relationship behavior is captured in parser-backed tests.
- The desired layout contract is clear before production code moves.

Primary gate:

- `cargo nextest run -p merman-ascii class`

Primary evidence:

- `class_parser_extension_star_renders_all_children`
- `class_parser_extension_chain_renders_each_relationship`
- `class_parser_crossing_relationship_layouts_are_explicitly_unsupported`
- `crates/merman-ascii/README.md`
- `cargo nextest run -p merman-ascii er`

## M2 - Shared Layout Boundary

Exit criteria:

- A shared terminal placement seam is used by both class and ER, or the lane records why sharing is
  rejected.
- Existing single-relation snapshots remain stable or are updated with a documented reason.

Primary gates:

- `cargo nextest run -p merman-ascii class`
- `cargo nextest run -p merman-ascii er`
- `cargo fmt --all --check`

Primary evidence:

- `crates/merman-ascii/src/relation_graph.rs`
- Class and ER single-relationship focused tests stayed green after routing through the shared
  seam.

## M3 - Class Multi-Relationship Rendering

Exit criteria:

- Class diagrams render at least one chain and one star/multi-edge topology.
- Every rendered relation is visible with marker and label semantics preserved.
- Dense or crossing layouts remain explicit diagnostics.

Primary gate:

- `cargo nextest run -p merman-ascii class`

## M4 - ER Multi-Relationship Rendering

Exit criteria:

- ER diagrams render at least one chain and one star/multi-edge topology.
- Cardinality and identifying/non-identifying semantics remain typed-model-driven.
- Dense or crossing layouts remain explicit diagnostics.

Primary gate:

- `cargo nextest run -p merman-ascii er`

Primary evidence:

- `er_parser_relationship_chain_renders_each_cardinality_and_label`
- `er_parser_relationship_star_renders_each_label_and_leaf_cardinality`
- `er_parser_crossing_relationship_layouts_are_explicitly_unsupported`
- `crates/merman-ascii/README.md`

## M5 - Public Gates And Closeout

Exit criteria:

- Public library and CLI ASCII gates pass.
- Support docs describe the shipped class/ER graph subset.
- Remaining topology gaps are closed or split into follow-ons.

Primary gates:

- `cargo nextest run -p merman-ascii`
- `cargo nextest run -p merman --features ascii`
- `cargo nextest run -p merman-cli --features ascii`
- `cargo fmt --all --check`
- `cargo clippy -p merman-ascii --all-targets -- -D warnings`
