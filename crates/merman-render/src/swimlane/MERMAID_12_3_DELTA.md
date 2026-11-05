# Mermaid 12.3 Swimlane delta

Implementation source: `mermaid@12.3.0`, commit
`2086904e76c692331e34a45d7cc3bc30860d040e`, materialized at
`repo-ref/mermaid-12.3.0`. Comparison source: Mermaid 12.2.0, commit
`1fb7869866dd619168d840291c9c54b4f129758b`, at `repo-ref/mermaid`.
The initial Merman worktree base was `d6cae254d910bd18cc7e6f4bf13db278392a53ba`.
Reference selection, locks, accepted SVGs, and Cargo verification belong to the coordinator.

## Source ownership

| Behavior | Mermaid source under `packages/mermaid/src/rendering-util/` | Local owner |
| --- | --- | --- |
| Shared-member lane pruning | `layout-algorithms/swimlanes/helpers.ts` | `prepare.rs`, parser-owned `dropped_subgraph_members` |
| Pools, nearest lanes, ordering | `layout-algorithms/swimlanes/{lanes.ts,phase2.options.ts}` | `lanes.rs`, `prepare.rs`, `working.rs` |
| Labels outside rank assignment | `layout-algorithms/swimlanes/{helpers.ts,phase4.coordinates.ts}` | `sugiyama.rs`, `sugiyama/labels.rs` |
| Deepest-first bounds, LR/RL stretch | `layout-algorithms/swimlanes/{helpers.ts,direction/lrTransform.ts}` | `bounds.rs`, `direction.rs` |
| Pooled title bands and label anchoring | `layout-algorithms/swimlanes/direction/{labelAnchoring.ts,materializedGeometry.ts}` | `direction/label_anchoring.rs`, `direction/materialized_geometry/` |
| Drawn ports, bounded spread, face balancing | `layout-algorithms/swimlanes/orthogonalRouter/router.ts` | `routing.rs`, shared `flowchart::bpmn` geometry |
| Annotation wrapping before measurement | `rendering-elements/shapes/bpmnAnnotation.ts` | `prepare.rs`, shared `flowchart::bpmn::wrapping_width` |
| Bend proximity, sibling face clearance | `layout-algorithms/swimlanes/direction/{geometry.ts,siblingSharedFaceRouting.ts}` | `direction/geometry.rs`, `direction/sibling_shared_face_routing.rs` |
| Arrival axis, heading, marker pull-back | `rendering-elements/edges.js` | `../svg/parity/flowchart/swimlane/endpoint_clip.rs`, shared `edge_geom/compute.rs` |
| Pool shell classes | `rendering-elements/clusters/{swimlane.js,../clusters.js}` | `../svg/parity/flowchart/swimlane/cluster.rs` |

`laneRole` and `laneIndex` are producer metadata. Mermaid filters these keys from author
metadata; this change does not introduce author pool syntax. A complete set of finite indices
overrides declaration order; partial coverage retains the upstream final reversal, including
the reversal after expanding pools into their child lanes. A marked group with no child groups
is a lane, not a pool. Shared membership removes only the lane that lost its final member and
has no incident edge. Authored empty lanes remain.

Working nodes retain the producer lane role independently of their mutable drawing shape.
Reusing an explicit `__swimlane_default__` pool for loose content changes its shape to
`swimlane`, as `helpers.ts::prepareLayoutForSwimlanes` does, without changing pool membership
or rank filtering. SVG dispatch still recognizes its child lanes from the producer role,
while the reused default group uses the lane shell. The combined preparation/rank/SVG
regression is `producer_pool_default_lane_reuse_preserves_membership_ranks_and_shells`.

Content measurement resolves the shape-specific wrapping width before both label metrics
and `NodeLayoutDimensionsRequest`. This uses the same helper as Dagre, ELK, and SVG emission;
the annotation handler's 160px override therefore survives global widths of 80px or 300px.
`annotation_layout_and_svg_use_shape_wrapping_width_for_html_and_svg_labels` checks both
label modes, layout/drawn bounds, HTML width, and SVG row consistency using inline input.

Labels retain measured dimensions and original edge ownership, but no longer create ranks or
ranking-only split edges. Gap reservation includes the measured flow extent plus 16 pixels on
each side. LR/RL divides this reservation by the content-node stretch before transforming it;
labels and groups cannot inflate that stretch. Same-rank labels reserve symmetric lane space.
Gap labels spread stably in declaration order. Row membership is indexed once rather than
rescanned for every label.

The BPMN shape owner supplies drawn dimensions. Swimlane ports use those dimensions while
layout, obstacle, and label measurements retain their existing complete boxes. No shared
shape implementation, random seed, or RoughJS generation policy is duplicated here.

## Integration boundary

The shared `edge_geom` dispatch now calls the family-owned docking helper and supplies the
actual end marker. The replaced 12.2 endpoint implementation and obsolete pin tests were
removed; the fixed-layout renderer integration regression remains.

The coordinator integrated `swimlane::is_lane_or_pool_cluster(ctx, cluster.id.as_str())`
into the shared cluster dispatcher, selecting the family shell for pool child lanes and nested
pools. The prepared cluster inventory now follows actual Swimlane lanes so authored empty
lanes retain their shells, title/color declaration slots and SVG order. Zero-width title rows
share `flowchart::zero_width_lane_title_rows` between measurement and emission; that owner is
also reachable in Agentflow-only builds. The shared layout core retains its router for
Mindmap's flat adapter independently of the Flowchart/Swimlane parser features.

The Mermaid logger's shared-member and nested-box warning strings are not new parser warning
facts. This render port consumes the existing parser provenance; it does not mutate canonical
warning facts or add an ambient logging dependency.

`anchoredNodes.ts` does not wire `pinAnchoredNodes`, `clearAnchoredOverlaps`,
`squareAnchoredEdges`, or `isAnchoredTo` into the production layout pipeline at this commit.
Merman does not add that missing pipeline, author anchor syntax, or a global attached-node
overlap exemption. Existing public Flowchart nodes carry no producer `anchorTo` metadata.
Source-only anchor scaffolding is not a claim of complete public anchor or BPMN language support.

## Observed evidence

On 2026-11-04 the installed candidate Mermaid 12.3.0 module graph was bundled with its
installed esbuild 0.28.1 and executed through Puppeteer in Headless Chrome 156.0.8078.4.
The default Puppeteer headless-shell download was absent; the run used the already installed
Playwright headless shell without installing or updating dependencies.

Twenty real browser renders succeeded: four directions each for plain/labeled cross-lane
edges, nested groups, shared-member pruning with an authored empty lane, and captioned BPMN
events. SVGs and observations are under `target/mermaid-12.3/swimlane-observable/`.
The browser runtime SHA-256 was
`59d4fc3b3a49696face5ab008b5a381368aae9f8b48e4b6af9eab2ef2cc77e93`.

Separately, eleven executions of the exact source core used explicit 60-by-40 node dimensions
and 400-by-130 label dimensions, plus producer pool/index inputs. These are executable
source-core observations, not browser measurements. Thirty-two assertions passed across the
two observation sets: same-rank content flow positions, pruning and empty-lane retention,
content ranks, and the exact label reservation (162 pixels in TB/BT, 432 in LR/RL).
`observations.json`, `source-core-observations.json`, and `assertions.json` preserve this
distinction. No Rust-to-browser parity comparison or accepted baseline refresh was run.

Follow-up read-only browser probes used the same installed-runtime digest, with observation
hooks at `positionNode` and `insertEdge`; no routing or positioning expression was changed.
For the unchanged `upstream_docs_basic.mmd`, `triage` to `investigate` had just two raw points,
`(36, 274.5)` and `(36, 651.5)`, so duplicate initial points are not a source invariant.
The synthetic label DOM transform matched its positioned node at `(36, 463)`. The
`upstream_7_car_sales_constr.mmd` label similarly matched `(650, 402.75)`; its raw route did
have duplicate terminal points in this browser profile. The regression now retains label
ownership, anchor, and viewport assertions without requiring a particular terminal pin form.
These are target-runtime observations, not expected native coordinates.

The hand-drawn selector migration follows `rendering-elements/clusters/swimlane.js`: RoughJS
body/title groups carry `swimlane-body` and `swimlane-title`, as classic rectangles do.
The theme regression now selects those groups explicitly; its original fill/stroke values
and evidence-count expectations remain unchanged.

The existing exact-source BPMN handler bundle was also executed with label
`Review this request before approval` and explicit `minWidth: 120`. At global wrapping
widths 80/300, HTML foreign-object width was 160 and node width was 182 in both cases.
SVG used the same two rows in both cases, with browser node width 161.625. This confirms
the shape override and mode-specific measurements; the Rust regression does not pin that
browser-specific SVG width. The source bundle is
`target/mermaid-12.3/bpmn/source-runtime.js`, SHA-256
`84d7e170111800d7949a634e7fbc5188c871f47f9f4cbefdb4cfcbdd1f9ff246`.
These follow-up probes did not run Rust tests or write fixtures/baselines.

## Coordinator verification

Five small authored-source regression inputs under `fixtures/swimlane/` are ready for the
coordinator's reference refresh. Each names its upstream test source in a Mermaid comment:

| Fixture suffix after `local_mermaid_12_3_` | Observable behavior |
| --- | --- |
| `label_ranks_tb.mmd` | Same-rank cross-lane labels and parallel labels sharing a rank gap |
| `nested_labels_lr.mmd` | Nested boxes and wide label reservation counted once in LR |
| `nested_bounds_rl.mmd` | Deepest-first nested containment in RL |
| `shared_members_tb.mmd` | Pruning the emptied lane while retaining authored-empty, incident-edge, and nonempty lanes |
| `drawn_ports_tb.mmd` | Captioned BPMN event ports, hexagon in/out faces, notch-pent and shared arrivals |

The installed candidate parsed all five as `swimlane`; FlowDB returned the expected BPMN,
hexagon and notch-pent shapes and the three distinct dropped-member records.
`target/mermaid-12.3/swimlane-fixture-input-admission.json` records their exact input SHA-256s.
This fixture admission invoked only `parse` and `getDiagramFromText`/`getData`; it did not
render or generate SVGs or goldens.

No Cargo command, commit, lock change, or accepted reference generation was run by this owner.
Read-only Rust parsing through `rustfmt --emit stdout` and scoped `git diff --check` were used.
The coordinator completed the cluster dispatch integration, generated actual semantic/layout
goldens and selected-runtime SVGs for all five inputs, and passed 49 focused Swimlane tests.
Two final regressions compare the retained empty-lane inventory and rounded hexagon arrival
commands directly with their current reference SVGs. The maintained focused commands are:

```text
cargo nextest run --locked -p merman-render --lib -E 'test(swimlane)'
cargo nextest run --locked -p merman-render --test swimlane_layout_test --test swimlane_svg_test --test swimlane_cluster_theme_test
cargo nextest run --locked -p merman --test swimlane_typed_render
cargo fmt --all -- --check
```

The fixed-layout clipping regression lives under
`svg::parity::flowchart::edge_geom::swimlane_points::tests`; the new docking matrix is under
`svg::parity::flowchart::swimlane::endpoint_clip::tests`. Existing cancellation, cumulative
work-budget, atomic route publication, and deterministic seed tests remain part of the first
command. Broader family snapshots and refreshed target SVG comparisons remain release gates
owned by the coordinator; historical Mermaid 11 baselines are not relabeled by this port.
