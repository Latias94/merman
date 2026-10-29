# ASCII Architecture Deepening — Evidence And Gates

Status: Completed
Last updated: 2026-10-29

AAD-010 through AAD-100 are complete. The commands and results below preserve the implementation
checks recorded through 2026-06-26; they are not fresh checks of the current source. The
[design](DESIGN.md) retains the architectural rationale, and the dated [journal](JOURNAL/) retains
the detailed investigations. Current support and remaining gaps belong to the
[ASCII crate documentation](../../../crates/merman-ascii/README.md) and
[gap registry](../../../crates/merman-ascii/ASCII_GAP_REGISTRY.md).

## Recorded Gate Set

### Styled Text/Cell Gate

```bash
cargo nextest run -p merman-ascii canvas color
```

Proves shared role-aware text behavior, trimming, and ANSI/HTML finalization.

### Graph Gate

```bash
cargo nextest run -p merman-ascii flowchart
```

Proves graph route planning and painting preserve supported flowchart behavior.

### Subgraph Direction Gate

```bash
cargo nextest run -p merman-ascii flowchart subgraph
cargo nextest run -p merman-ascii graph_fixture
```

Proves the shipped local-direction subset and copied-fixture inventory stay consistent.

### Boundary Routing Gate

```bash
cargo nextest run -p merman-ascii flowchart subgraph
```

Recorded for cross-boundary mixed-direction routing slices with parser-backed subgraph fixtures
and targeted route-plan tests in AAD-090.

### Relation Graph Gate

```bash
cargo nextest run -p merman-ascii class er
```

Proves class and ER relation adapters preserve current rendering behavior.

### Sequence Gate

```bash
cargo nextest run -p merman-ascii sequence
```

Proves event planning preserves lifecycle, activation, visibility, and control-frame behavior.

### Package Gate

```bash
cargo nextest run -p merman-ascii
```

Proves all ASCII family regressions still pass after the architectural seams are introduced.

### Closeout Gates

```bash
cargo fmt --all --check
cargo clippy -p merman-ascii --all-targets -- -D warnings
git diff --check
```

Proves formatting, lint cleanliness, and whitespace hygiene.

## Evidence Log

- 2026-05-30 — AAD-010 started. Workstream opened for the five ASCII architecture deepening targets.
- 2026-05-30 — AAD-010 passed `git diff --check -- docs/workstreams/ascii-architecture-deepening`.
- 2026-05-30 — AAD-020 introduced shared `StyledCell`/`StyledLine` substrate and migrated sequence
  and XYChart line buffers. Passed `cargo nextest run -p merman-ascii canvas color`,
  `cargo nextest run -p merman-ascii text`, `cargo nextest run -p merman-ascii sequence`,
  `cargo nextest run -p merman-ascii xychart`, `cargo nextest run -p merman-ascii`,
  `cargo fmt --all --check`, and `cargo clippy -p merman-ascii --all-targets -- -D warnings`.
- 2026-05-30 — AAD-030 introduced a graph route-plan seam for top-down direct routes. Passed
  `cargo nextest run -p merman-ascii top_down_direct`, `cargo nextest run -p merman-ascii flowchart`,
  `cargo nextest run -p merman-ascii`, `cargo fmt --all --check`, and
  `cargo clippy -p merman-ascii --all-targets -- -D warnings`.
- 2026-05-30 — AAD-040 deepened relation graph adapters by moving `RelationGraphLine` onto
  `StyledLine`, centralizing box row construction, relation line merging, and centered relation text
  writing. Passed `cargo nextest run -p merman-ascii relation_graph`,
  `cargo nextest run -p merman-ascii class er`, `cargo nextest run -p merman-ascii`,
  `cargo fmt --all --check`, and `cargo clippy -p merman-ascii --all-targets -- -D warnings`.
- 2026-05-30 — AAD-050 introduced `SequenceEventPlan` for activation counts, actor visibility,
  lifecycle visibility transitions, and control frame ordering state. Passed
  `cargo nextest run -p merman-ascii event_plan`, `cargo nextest run -p merman-ascii sequence`,
  `cargo nextest run -p merman-ascii`, `cargo fmt --all --check`, and
  `cargo clippy -p merman-ascii --all-targets -- -D warnings`.
- 2026-05-30 — AAD-060 added `crates/merman-ascii/ASCII_GAP_REGISTRY.md` and linked it from the
  ASCII README. Passed `git diff --check -- crates/merman-ascii docs/workstreams/ascii-architecture-deepening`.
- 2026-05-30 — AAD-070 final verification passed `cargo nextest run -p merman-ascii` with 163
  tests, `cargo fmt --all --check`, `cargo clippy -p merman-ascii --all-targets -- -D warnings`,
  and `git diff --check`.
- 2026-05-30 — AAD-070 review found no blocking workstream compliance or code-quality findings.
  Residual route/feature expansion risk is intentionally tracked in `crates/merman-ascii/ASCII_GAP_REGISTRY.md`.
- 2026-06-01 — AAD-080 resumed the lane to ship the first honest `FlowSubgraph.dir` subset:
  canonical `LR` subgraphs inside canonical `TD` roots for fully internal routed edges.
  Passed `cargo nextest run -p merman-ascii flowchart subgraph`,
  `cargo nextest run -p merman-ascii graph_fixture`, and `git diff --check`. The subset landed in
  commit `3dbd5a3b0`.
- 2026-06-02 — Investigated cross-boundary mixed-direction routing against
  `repo-ref/beautiful-mermaid/src/layout-engine.ts`. Confirmed the next durable step is a boundary
  routing seam rather than more layout heuristics. Experimental layout-only attempts were discarded
  instead of being kept as unstable debt.
- AAD-090's completed routing seam uses explicit `DrawEdgeRequest` and `GridRouteOptions` request
  objects, boundary classification and segment markers, parser-backed boundary fixtures, and
  planned canvas extents. The completed task record describes this bounded scope; it does not
  establish unrestricted mixed-direction parity.
- 2026-06-02 — Added route-plan coverage for TD same-rank merge edges and Unicode turn-glyph
  direction correctness after a reported flowchart branch/merge regression. Added parser-backed
  coverage for `flowchart TD` branch merging so the `B -->|No| D` bend uses a connected `┐` and
  the `C --> D` same-rank edge is not silently dropped.
- 2026-06-02 — Clarified sequence actor mirroring as a contract issue rather than a default parity
  bug: Mermaid 11.15 defaults `sequence.mirrorActors` to `false`, while `merman-ascii` now exposes
  `AsciiRenderOptions::with_sequence_mirror_actors(true)` and `merman-cli --sequence-mirror-actors`
  for terminal output that wants bottom participant boxes.
- 2026-06-02 — Fixed class namespace qualified relation endpoints in `merman-core` so relations
  like `Platform.FFI.DartBinding --> Platform.Core.Renderer` resolve to already declared
  namespace member classes instead of creating duplicate fully qualified top-level classes. Updated
  affected class parser goldens for the intentional duplicate-class removal.
- 2026-06-02 — Fixed ER/class shared layered relation routing for spanning edges that cross
  intermediate boxes. The route lane now chooses a clear side around intermediate boxes, so the
  reported `PRODUCT ||--o{ ORDER_ITEM` edge no longer draws through `ORDER` attributes.
- 2026-06-02 — Verification passed `cargo nextest run -p merman-ascii`,
  `cargo nextest run -p merman-core --lib`, `cargo nextest run -p merman-core
  namespace_qualified_relation_endpoints_resolve_to_declared_classes`,
  `cargo fmt --all --check`, `cargo clippy -p merman-ascii --all-targets --no-deps -- -D warnings`,
  `cargo clippy -p merman-cli --features ascii --all-targets --no-deps -- -D warnings`,
  `cargo nextest run -p merman-cli --features ascii`, and `git diff --check`.
  `cargo nextest run -p merman-core` still fails on unrelated fixture
  `fixtures/flowchart/stress_flowchart_edge_label_position_064.mmd` because the current parser
  emits node `labelType=markdown` while the checked-in golden still expects `text`.
- 2026-06-26 — AAD-100 removed the duplicate graph route extent helper. `RoutePlan` now owns the
  canvas extent derived from selected route cells, planned labels, and explicit route-family minimum
  extents for back-lane width contracts, rather than global right padding. Commit `a48417a8e`.
  Passed `cargo nextest run -p merman-ascii`,
  `cargo clippy -p merman-ascii --all-targets -- -D warnings`, and `git diff --check`.
- 2026-06-26 — AAD-100 introduced `graph::topology::GraphGroupTopology` so routing boundary
  classification and group layout share group id, node membership, recursive member, and nesting
  depth lookup instead of rebuilding local parent/member maps. Commit `3c1ee58c1`.
  Passed `cargo nextest run -p
  merman-ascii`, `cargo clippy -p merman-ascii --all-targets -- -D warnings`, and
  `git diff --check`.
