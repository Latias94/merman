---
type: "Work Progress"
title: "Mermaid 12 recursive provider scopes and corrected label projection"
description: "Ordinary mixed hierarchy executes through provider scopes; independent review corrected inline and zero-size edge labels."
timestamp: 2026-09-20T09:17:36Z
record_id: "5a66d2154b834776b5a5589bdfbaa008"
producer_id: "codex-mermaid12"
run_id: "session-01a0bd10"
related_plan: "docs/plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md"
git_branch: "refactor/mermaid-12-alignment"
git_commit: "df71f0586"
supersedes: "dfc75ff054d84a2e88dcb3da99270a4e"
---

# Summary

The complete Mermaid 12 U1-U15 / R1-R12 goal remains active and incomplete. This turn made
concrete progress with `172e667ab` (correct edge-label projection) and `df71f0586` (ordinary
recursive provider scopes). The prior turn was progress but its label-default conclusion was
wrong; the independent review below corrected both the implementation and its reference input.
Production bundle/defaults remain 11.17.2. No new root names are registered in the renderer yet.
No push, PR or release.

# Details

## Correction to the preceding checkpoint

Read-only reviewer `/root/review_flat_dispatch` (Averroes) found three genuine projection errors
in `46d3ae887`. All were fixed in `172e667ab`:

- Mermaid12 `render.ts:1123-1133` explicitly creates one inline CENTER label on **every** edge.
  Force/Stress must use `inline=true`. The prior claim that false was the correct fallback is
  retracted: the old oracle omitted Mermaid's label layoutOptions and validated a different input.
- An unlabeled edge still receives a zero-size label. Force imports it as a particle with
  dimensions clamped to at least 1; omitting it changes Force and Stress node placement.
- MrTree's first ELK node label determines special synthetic-root handling. A node ID must not
  be passed as label text: ID `SUPER_ROOT` incorrectly suppressed routes. Leaves now pass no
  ELK label text; group title text is separately represented in the next commit.

`target/mermaid-12-admission/flat-adapter-oracle.cjs` and `.json` now include explicit inline
options and the second edge's empty label. Committed numerical cases were updated from that
actual target output, not adjusted to fit Rust. Source kernel semantics did not change.
Six focused tests passed after correction (`0e797fdd-865c-4783-a6a6-8cd9cacd8715`), including
renaming a valid MrTree node to `SUPER_ROOT` without changing geometry or routes.

The reviewer found no additional work-error classification issue. This was a review of the
flat dispatch commit, not independent approval of the subsequent hierarchy implementation.

## Recursive scope implementation

- `ScopePlan` records a provider. Non-Layered parents always separate nonempty groups; groups
  without a direction resolve to Layered, while a direction selects the diagram provider.
  Standalone default Layered groups below non-Layered parents use RIGHT; the resulting
  direction is propagated when planning deeper scopes.
- Postorder scheduling executes each scope through the actual provider. `ElkInputGraph` is
  only materialized in the Layered branch; other scopes receive measured Graph values with
  completed child extents. Empty groups become ordinary measured leaves for those providers.
- `flat::FlatLayout` returns public geometry, provider extent and packing content shifts.
  Flattening adds those shifts to child scope origins, affecting all descendants/edges/labels.
  Actual child layout dimensions replace stale input group dimensions in the non-Layered branch.
- `flat::ScopeContext` distinguishes root from ordinary container options. Ordinary non-Layered
  containers use padding24/node spacing50; root Rectpacking alone receives its root preset.
  **Do not add the label height to ordinary non-Layered top padding.** Real directional Box and
  Force container oracles confirm children start at y24. Layered separately reserves its title.
- `Node::label_text: Option<String>` retains container text for MrTree's source identity.
  Flowchart, Class and ER constructors populate it. Leaves have no ELK node label array.
  MrTree also receives ordinary child containers' padding24 rather than leaf default20;
  its importer/normalization reads child padding, not just parent padding.
- A non-Layered scope with boundary segments returns
  `UnsupportedCrossProviderEdge { edge_id }` for now. It does not fabricate ports or silently
  suppress the edge. This replaces the former blanket NonLayeredHierarchy error; ordinary
  nested groups and edges attached to group frames now work.
- No explicit container algorithm metadata or cross-boundary downgrade has been connected yet.
  Full micro-layout/minimum-size projection, special container presets and renderer registration
  remain pending. Do not claim all mixed-hierarchy behavior from the current eight examples.

The nested reference uncovered a pre-existing common port-placement error. For a 20px side,
portsSurrounding12 and base24, source port y is10.8 rather than10. Source
`PortPlacementCalculator` keeps the initial coordinate when the content cell is too small and
overhang is disabled. `common/nodespacing.rs::placement_start_and_spacing` now implements that
branch, including negative inter-port spacing when required, rather than recentering/clamping.
For a single port the source's unused divide-by-zero spacing is avoided; placement is identical.

## Verification

Actual target nested inputs/outputs: `target/mermaid-12-admission/nested-adapter-oracle.{cjs,json}`.
Eight cases cover Box/Force/Rectpacking/MrTree roots, each with an ordinary default-Layered
container and a directional container selecting the root provider. They include title dimensions,
an internal a->b edge and an edge attached to the group's frame. All absolute node centers and
sizes match; two edges are retained. Committed evidence is in `flat/tests.rs`.

Because common port placement changed, both source and adapter suites were run:
`cargo nextest run -p merman-elk-layered -p merman-layout-elk --status-level fail --final-status-level fail`
passed **448 tests**, run `de283aa5-3878-45d4-a862-aef5fae8aaac`. After naming the context enum and
refining the temporary cross-provider error, final affected adapter tests passed (7 tests,
`2591d1cf-3348-4834-88a2-457a36535c63`). Renderer compilation with `layout-elk`, cargo fmt check,
diff check and the license contract verifier passed. No dependency/feature/legal projection
change. Do not rerun these unchanged suites without new changes or a concrete concern.

# Next Action

Continue the source-backed container contract in predecessor record
`dfc75ff054d84a2e88dcb3da99270a4e` (its hierarchy findings remain valid; its false inline-default
claim does not). Add metadata/direction/absent algorithm provenance and Mermaid's pre-layout
cross-boundary downgrade; then explicit container option projection, provider micro/resize,
and cross-provider boundary handling. A Node currently retains only direction/handling/title;
there is still no typed metadata algorithm or raw container padding/minimum contract.

Flowchart's current FlowSubgraph model does not retain metadata directly; inspect the parser's
render context before adding container metadata. Mermaid12 flowDb.ts:154 routes already-declared
subgraph metadata into the group and :1254 forwards it to layout. Do not lose this at the model
projection boundary. Source `clearContainerAlgorithmOptions` also removes padding and does not
restore ordinary padding24; preserve that real behavior when implementing downgrade.

Remaining full-goal work is unchanged: U2 defaults/themes; State/Requirement/Mindmap integration;
remaining U5 frame/degenerate-anchor/marker/line-hop processing; family geometry/themes;
Agentflow/Usecase; editor/bindings; default/lean feature and distribution closure; genuine target
baseline admission; final size/perf and independent review. Kernel and adapter tests do not
discharge those deliverables. Renderer missing-section painting is especially needed before
advertising packing providers.

# Citations

- Commits `172e667ab`, `df71f0586`.
- `docs/plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md`.
- Pinned Mermaid `98a0945418c76238f15df2afaddbba4272656c3b`; ELK
  `62d5909f96fad541bc101ad52dabaece6b7eab7e`; elkjs0.9.3.
- Predecessor record `dfc75ff054d84a2e88dcb3da99270a4e` contains detailed source line references.
- Legacy engineering rollups remain unadopted; do not overwrite them or repeat migration attempts.
