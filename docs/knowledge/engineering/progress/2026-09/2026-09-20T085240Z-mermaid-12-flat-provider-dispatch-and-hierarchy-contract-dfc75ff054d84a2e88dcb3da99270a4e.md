---
type: "Work Progress"
title: "Mermaid 12 flat provider dispatch and hierarchy contract"
description: "Seven provider projections match actual elkjs; source findings define the remaining mixed hierarchy integration."
timestamp: 2026-09-20T08:52:40Z
record_id: "dfc75ff054d84a2e88dcb3da99270a4e"
producer_id: "codex-mermaid12"
run_id: "session-01a0bd10"
related_plan: "docs/plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md"
git_branch: "refactor/mermaid-12-alignment"
git_commit: "46d3ae887"
supersedes: "6ca99d794c984fc4911a7c54aad95637"
---

# Summary

The complete U1-U15 / R1-R12 goal remains active and far from complete. This turn made concrete
progress in `46d3ae887`: seven additional providers now execute through the measured flat graph
adapter, with real elkjs comparison. Root/container renderer registration, mixed hierarchy,
micro layout for groups and missing-section painting remain next. Production bundle/defaults
still select 11.17.2. No push, PR or release. Do not promote the partial staged target bundle.

# Details

## Implemented boundary and verification

- `LayoutOptions::algorithm` selects Layered (default), Box, Rectpacking, Force, Stress,
  MrTree, Radial or SporeOverlap. `flat.rs` projects measured nodes/edges into actual kernels,
  converts top-left node coordinates to adapter centers and retains label top-left positions.
- Non-layered parent relationships return typed `NonLayeredHierarchy` for now, rather than
  silently flattening or invoking Layered. Phase diagnostics reject non-Layered selection.
  The renderer explicitly still constructs Layered options; no new loader is advertised yet.
- Missing kernel sections remain empty point chains, preserving the downstream semantic
  distinction from routed straight lines. Packing does not invent routes or label positions.
- Rectpacking uses the target root preset. Other algorithms use provider defaults: Layered's
  `spacing.baseValue=40` must not override Force spacing80, MrTree spacing20, etc.
- Stress labels actually use ForceOptions' inline=false fallback through FLabel.refreshPosition,
  despite Stress.melk advertising true. Actual elkjs comparison exposed and fixed this projection
  error. Do not change the expected oracle or reintroduce the metadata default blindly.
- `algorithms::resolve_seed` adds distinct Force and SPOrE operation domains while preserving
  signed nonzero Java seeds and the existing Layered seed stream. Stress initialization uses
  Force's domain. SPOrE replaces Math.random with operation-owned deterministic values (raw
  calls use configured nonzero seed); this is replay policy, not browser random sample parity.
- MrTree work APIs accept `&mut dyn WorkControl` throughout. Adapter work-error classification
  covers nested Stress/Force and Rectpacking/Box errors as well as direct provider errors.

Reference script/output: `target/mermaid-12-admission/flat-adapter-oracle.{cjs,json}`. It runs real
elkjs0.9.3 on three differently sized nodes with a labeled chain, normal micro layout enabled,
and target root options. All seven provider node sizes/centers, routes and labels match.
Committed numerical evidence is `crates/merman-layout-elk/src/flat/tests.rs`.

Final scoped run:
`cargo nextest run -p merman-layout-elk -p merman-elk-layered -E 'package(merman-layout-elk) | test(algorithms::mrtree) | test(algorithms::tests) | test(random::tests)'`
passed 60 tests, run `c7ccb68e-fa23-4e16-b08e-d98fa8e129de`. This includes existing Layered hierarchy
and random-policy regressions. `cargo check -p merman-render --features layout-elk`,
`cargo fmt --all --check`, `git diff --check`, and third-party license verification passed.
No new dependencies/features or source-license boundary changes. Final independent code review
remains pending; the source-contract subagent performed read-only upstream research this turn.

## Source contract for the next integration step

Read-only subagent `/root/algorithm_source_contract` (Avicenna) verified the following against
Mermaid `98a0945418c76238f15df2afaddbba4272656c3b`, ELK0.9.1
`62d5909f96fad541bc101ad52dabaece6b7eab7e` and one actual elkjs hierarchy oracle.
All Mermaid line references below are in
`repo-ref/mermaid/packages/mermaid/src/rendering-util/layout-algorithms/elk/render.ts`.

1. Root registry `algorithms.ts:9`: `elk` maps Layered; root names additionally include Stress,
   Force, MrTree, SporeOverlap, Box, Rectpacking. No root `elk.layered` or `elk.radial`.
   Container allowlist at render.ts:287 includes all eight exact `elk.*` algorithm strings.
2. Metadata algorithm at :462 takes precedence over dir. With valid metadata, node.dir is not
   simultaneously written. Without valid metadata but with dir, :488 sets the algorithm to the
   **diagram's** algorithm, not the nearest ancestor's. Without either, no algorithm is set.
   ELK `core/.../data/LayoutAlgorithmResolver.java:42,119` resolves unset nonempty groups to
   **Layered**, not a non-Layered parent's algorithm. Actual elkjs root Layered/Box/Force all
   produced identical internal default-Layered group geometry for a->b (104x44 group,
   children (12,12)/(62,12), routed internal edge); root only changed group placement.
3. `RecursiveGraphLayoutEngine.java:169-202`: INCLUDE_CHILDREN only absorbs hierarchy when the
   provider supports COMPOUND/CLUSTERS. Explicit SEPARATE or a different explicit algorithm
   starts another scope. Other providers recursively lay out each direct nonempty child (:389).
   Current adapter `HierarchyIndex::build` only considers hierarchy enums; extend it with
   resolved algorithm/capability and retain metadata/direction/absent provenance.
4. Cross-boundary processing happens **before** layout/scope planning (:1191-1245). Endpoints
   with different parentId trigger common-ancestor lookup; walk both endpoint chains including
   that ancestor. Effective metadata algorithm + SEPARATE clears algorithm-specific overrides,
   reapplies ordinary title minimum and sets INCLUDE. Direction-sourced algorithms survive.
   `clearContainerAlgorithmOptions(:267)` removes algorithm, nodeSize constraints/minimum,
   aspectRatio, contentAlignment, expandNodes, padding, and all RECTPACKING_OPTIONS. It restores
   base24/nodeNode50 but **does not restore padding24**. Preserve this actual behavior. Do not
   route cross-boundary metadata Box/Force scopes independently using invented boundary ports.
5. Ordinary groups (:393): base24/nodeNode50/edgeNodeBetweenLayers30/edgeEdge20/padding24,
   inside top-center title and independent container placement/cycle preset. Painted title
   minimum width=label.width+node.padding, height0 (:347,359). Explicit metadata (:463):
   minWidth=labelW+2*node.padding, minHeight=labelH+30, padding top=labelH+15, other15,
   aspect2, H_CENTER/V_TOP, expandNodes, SEPARATE; applies even to explicit Layered.
   Rectpacking uses top=labelH+10, other10, minHeight=labelH+20 and its shared preset/aspect1.6.
   Nonempty groups have input x/y/width/height deleted at :1158. Current materialization maxes
   source dimensions with child extent; replace that with actual explicit minimum semantics.
6. Micro layout is provider-specific: Force before import; Stress noninteractive through Force,
   interactive directly; MrTree/Radial before layout; Rectpacking before packing and after
   expansion. Box has no NodeMicroLayout; SPOrE computes only margins. Source common
   `NodeMicroLayout.java:55` sorts ports, resolves labels/node size and computes margins.
   `ElkUtil.resizeNode:253` applies effective minima, moves ports/labels per flags, then fixes
   constraints. A nonpositive minimum axis can resolve to DEFAULT_MINIMUM_SIZE=20 (:347,69).
   Force/Box/Rectpacking resize(false,true), MrTree(false,false), SPOrE(true,true).
7. Box expansion returns content shifts from the child's contentAlignment
   (`BoxLayoutProvider.java:238-263`). Apply these to descendant nodes, internal edges and labels
   in addition to updated group dimensions. Current flatten only adds group top-left offsets.
8. Missing-section paint branch (:1797-1830) applies to every absent/empty section list: final
   absolute endpoint centers -> shape clipping -> linear curve -> label at clipped-line midpoint.
   It returns before marker-terminal-length processing. Routed two-point sections still use
   normal center append/clipping/marker length/rounded curve/algorithm label positions (:1834+).
   Keep these cases distinct when extending renderer output.

# Next Action

Implement algorithm provenance and cross-hierarchy downgrade first, then algorithm-aware scopes;
reuse postorder scheduling and flattening but use ElkInputGraph only for the Layered branch.
Introduce provider-specific micro/resize work inside the EPL source-port boundary as needed,
carry actual child extents and content shifts, then register renderer root/container choices
and missing-section painting. Test mixed algorithms with real target examples, not fabricated
boundary geometry. Reuse completed kernels and avoid rerunning unchanged broad suites.

The full goal still includes U2/defaults/themes, State/Requirement/Mindmap, remaining U5 frame/
degenerate-anchor/marker/line-hop behavior, family geometry/themes, Agentflow/Usecase, editor/
bindings, feature/distribution closure, genuine target baseline admission, final size/perf and
independent review. None is discharged by this flat adapter checkpoint.

# Citations

- Implementation commit: `46d3ae887`.
- Plan: `docs/plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md`.
- Predecessor: record `6ca99d794c984fc4911a7c54aad95637`.
- The legacy engineering rollups remain unadopted. Do not retry rendering them or overwrite
  user-owned root views; immutable progress shards are the current continuity source.
