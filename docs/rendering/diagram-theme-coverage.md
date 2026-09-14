# Diagram Theme Coverage

This is a migration snapshot, not a stable support contract. It distinguishes a direct typed
consumer, an executable legacy compatibility route, and an explicitly Unsupported mechanism.
A compiled theme is opt-in; the removed `PresentationTheme`/`HostTheme` APIs are not aliases.

## Snapshot: 2026-09-14

Class Text is now a partial typed surface and no longer enters the family bridge dispatch.
Source `bb937ecb1` recorded 3097 scoped Release owner tests (two existing skips),
following the earlier 3170-test owner run, plus Web/WASM package smoke. Its recorded
transport matrix passed Node 40/40, Web catalog 8/8, Typst 41/41, Native C ABI and UniFFI
2/2, and Flutter ABI 3 contract verification. Flutter one-shot theme authoring now routes all three
authoring operations through the theme-specific resource admission path.
The [Flutter budget matrix](../knowledge/engineering/verification/2026-09-14-flutter-authoring-budget-matrix.md)
subsequently rebuilt the macOS arm64 Native Assets library at `44d81d033` and exercised valid
and rejected budgets for all three operations through both consumers. Independent old-wrapper
mutations fail the new regression. The same source passed 3158 scoped Release tests (two skips)
in an independent clean checkout, including both Block and Class retirement integration targets.
The original Class convergence source `2a92c56b6` passed 3170 private Release owner tests (two existing skips) and 124
additional Class SVG, authoring and route-runtime tests in a clean checkout. Seven isolated
text channels have native Solid/Transparent controls across three looks and two selectors.
KTD17 v87 covers 486 routes and 754 route-profile witnesses; the executable legacy inventory
now contains only Block's 20 routes. Block Edge adds eight static fill/stroke routes with
classic, neo, and handDrawn witnesses. Paint is bound to each emitted edge path; Edge.fill remains
a stroke fallback only when stroke is unspecified. The old Edge-to-Marker projection is removed.
Block NodeLabel's preceding four static fill routes remain bound to individual node terminals.
Source `a902b5d53` passed 3218 scoped Release owner tests in both the main worktree and an
independent clean checkout (two existing skips). The main worktree also passed the full SVG
structure gate and rebuilt Flutter Native Assets authoring with all 17 revision-87 support
queries and the three-operation budget matrix. See the
[Block Edge record](../knowledge/engineering/verification/2026-09-14-block-edge-direct-paint.md).
These are scoped migration results, not C7a eligibility or complete Class mechanism support. See the
[Class Text convergence record](../knowledge/engineering/verification/2026-09-14-class-text-convergence.md).

The earlier `523ce8881` cutover produced 38 owner-test failures. Subsequent writer-emission,
shadowed-rule, route-authorization, support and dispatch repairs closed those failures in the
owner combination above. Historical verification records retain their own source and scope;
they do not describe the current inventory.

Historical prerequisite (`c16be4559`): Class's text terminal inventory retained color
ownership alongside independent font facts. NodeLabel and namespace Title share those facts, including explicit rejection of
empty/source-owned/unknown text as inherited paint. Source `c16be4559` passed 3281 private
Release owner tests in both the main worktree and a clean checkout (two existing skips), plus
main-worktree Class structure and Clippy checks. This record predates the completed Class Text
bridge retirement described above. See the [shared terminal verification record](../knowledge/engineering/verification/2026-09-13-class-shared-text-terminal-facts.md).

Historical author-order repair (`00174dc2a`): Class NodeLabel shares generic Text author order
in all paint resolution paths, including per-node ordinal caching. Source `00174dc2a` passed 3278 private Release owner tests in both
the main worktree and a clean checkout (two existing skips), plus main-worktree SVG structure
and Clippy checks. At that revision, generic Text remained legacy/incomplete where unproved;
that repair did not change support or retirement counts. The subsequent Class Text retirement
supersedes that status. See the [author-order repair record](../knowledge/engineering/verification/2026-09-13-class-text-node-author-order.md).

The evidence requirement index repair at `d92dee3ad` passed 3,273 private Release owner tests
(with two existing skips), SVG structure comparison and actual Typst package checks in a clean
checkout. It removes quadratic key deduplication while preserving ordered requirements and
legacy/native ownership. See the [structural repair record](../performance/theme_evidence_requirement_index_2026-09-13.md).
Support revision, bridge counts and qualification cells are unchanged; C7a/C7b remain open.

The final Typst publish artifact at clean source `f151192c9` passed all 15 revision-86 support
vectors, two materializations, three authoring/resource errors and the shared preset catalog
through its actual WASM ABI. The pinned Typst CLI passed 22 compilations and nine expected failures;
55 scoped Rust tests also passed. All four Typst size budgets still fail. See the
[Typst artifact record](../knowledge/engineering/verification/2026-09-13-typst-theme-artifact-revision86.md)
for the exact payload identity and limits. This does not promote public qualification cells.

All five Web packages were rebuilt from current source `5834e84e5`, packed, installed offline and
checked through the package smoke matrix and DOM safety smoke. Full/render each passed the 15
revision-86 support
vectors and the shared authoring/catalog task. All twenty size-budget checks still fail; this
updates installed-consumer evidence without closing C7a. See the
[Web consumer record](../knowledge/engineering/verification/2026-09-13-web-theme-consumers-revision86.md).

C5 closed at `ff486589c` after 1128/1128 private Release owner tests passed in a clean checkout.
The scope includes all 41 parser variants through typed preparation and SVG writers, all 33 logical
family theme scopes, bounded program/evidence work, primary-family direct consumers and authorized
route/typography retirement. Minimal-feature catalog tests passed 46/46 on the same source.
See the [C5 closure record](../knowledge/engineering/verification/2026-09-13-c5-catalog-execution.md)
for production-equivalent default-output evidence and precise limits. This does not close C7a
artifact qualification/public rollout or C7b long-tail migration and provider/probe retirement.

The Class retirement source at `f1046a9cf` passed 144/144 private Release theme acceptance tests,
276/276 focused renderer/support tests, 15/15 compiled Rust transport/authoring tests, and the full
SVG structure gate. These results do not replace installed-package or final artifact-profile
verification. See the [retirement record](../knowledge/engineering/verification/2026-09-13-class-background-retirement.md)
for exact scope and clean-checkout evidence.

The earlier cross-platform verification at commit `ca257394a` passed the private theme acceptance harness
138/138, the Node contract suite 104/104, the cross-transport support and resource golden
checks, and the acceptance-boundary and representative artifact-closure checks. Since that
verification, the current branch has also passed the Flutter Native Assets authoring witness,
the preset qualification feature-recipe guard, the legacy projection/route-cutover check
(3/3), and the complete repository script gate (582/582), and a scoped Release theme acceptance
harness (117/117 with PNG and Cytoscape layout features). These incremental checks keep the
ledger aligned but do not promote the remaining legacy routes or close C7a/C7b. A clean detached checkout at
`8e3831d11` regenerated the schema-3 qualification record and passed its replay check.
That record covers the qualification executable; its `cli` field is null, so it does not
attest a packaged CLI. The commit identifies the tested source, not a moving HEAD.
Earlier C6 runtime and preset qualification sub-gates also passed 4/4.
The earlier optimized renderer regression for family/bridge evidence paths passed 213/213.

The source inventories, not this summary, authorize rendering and retirement:

| Inventory | Snapshot | Source |
| --- | --- | --- |
| Families with at least one direct typed surface | 33/33 | `crates/merman-render/src/diagram_theme/family_mechanism_matrix.rs` |
| Families without a family-owned Legacy route | 32/33 | `crates/merman-render/src/diagram_theme/legacy_family_theme_bridge.rs` |
| Families with executable Legacy routes | 1/33; 20 routes | Block 20. Class Text bridge dispatch and four legacy routes are retired; exact counts are guarded by renderer and acceptance tests. |
| Base FontStack | 32 Typed / 0 Legacy / 1 Unsupported | Mechanism matrix |
| Base FontSize | 18 Typed / 0 Legacy / 15 Unsupported | Mechanism matrix |
| Public support claims | Revision 87 | `crates/merman-render/src/diagram_theme/support_manifest.rs` |
| KTD17 scalar cutover | v87; 486 routes / 754 route-profile witnesses | `crates/merman-theme-acceptance/src/cutover_manifest.rs` and `cutover.rs` |
| KTD23 historical retirement | v9; 80 routes / 160 value probes | `crates/merman-theme-acceptance/src/route_retirement_manifest.rs` |

KTD17 v87 replaces Block's eight static unqualified/Default solid/transparent Edge fill/stroke
routes. Actual path receipts bind source identity, exact layout-derived geometry, paint and
single emission; missing, duplicate or damaged terminals cannot certify a route. Legitimate
zero-length self-loops remain renderable and do not claim applied paint. Source lineColor and
matching class styles retain ownership. Marker paint remains a separate legacy surface;
KTD23 v9 is unchanged because Edge paint has a typed replacement.

KTD17 v86 added Block NodeLabel's four static unqualified/Default solid/transparent routes.
Terminal checks cover source/config ownership, empty labels, overwritten rules and unsupported
sibling facets. This leaves generic Block Text and its compatibility provider active.

KTD17 v85 added Class Text's four static unqualified/Default solid/transparent routes.
The preceding v84 Title routes continue to belong to namespace labels. Source ownership,
shared Text/role author order, and residuals for unsupported requests remain explicit.
Removing Class from bridge dispatch does not make its ordinal, gradient, effect or other
unsupported mechanisms portable. See the convergence record above for current verification
and the [namespace Title record](../knowledge/engineering/verification/2026-09-13-class-namespace-title-cutover.md)
for that earlier tranche.

KTD17 v83 replaces Class's eight scalar Cluster routes after the Edge cutover: unqualified/Default
fill and stroke, each solid/transparent. Ordinary, extracted, and ELK namespace rectangles receive
typed paint with independent facet receipts. The core theme program retains the source ownership
of `clusterBkg` and `clusterBorder`. The
[Class Cluster verification record](../knowledge/engineering/verification/2026-09-13-class-cluster-scalar-cutover.md)
also records the corrected build provenance and source-indexed relation receipts.

KTD17 v82 replaces Class's six remaining scalar Edge routes: unqualified/Default
fill and Default stroke, each solid/transparent. Fill supplies relation, attached-note connector,
and referenced-marker stroke only when stroke is unspecified; Clear and unsupported stroke block that fallback.
The terminal receipt binds the winning rule and source property, including source-indexed note
connector checkpoints. Notes do not extend relation ordinals or width. Class no longer executes
edge or marker-fallback bridge assignments; source `lineColor` keeps ownership. The added
Classic/HandDrawn native witnesses observe Edge.fill in the SVG stroke channel. Public support
revision 84 reports a partial typed Edge.fill surface. Source `e696949f6` passed 445/445
private Release tests in a clean checkout, including all 678 native route-profile witnesses.
KTD23 v9 is unchanged. Class Title still
has a real namespace-label consumer and is not an unused projection. See the
[Class edge record](../knowledge/engineering/verification/2026-09-13-class-edge-scalar-cutover.md).

KTD17 v81 replaces Block's four unqualified/Default scalar EdgeLabelBackground.fill
routes with typed stylesheet and visible-label consumers. The background bridge projection is
removed; explicit source configuration retains ownership. Classic, Neo, and HandDrawn native
route witnesses cover solid and transparent fills. Public support revision 83 reports a partial
typed surface; unsupported ordinal and sibling facets remain residuals. This is a typed replacement,
so KTD23 v9 is unchanged. Revision-82 installed results remain historical; current consumer
observations are listed below. See the
[Block background record](../knowledge/engineering/verification/2026-09-13-block-edge-label-background-legacy.md).

KTD23 v8 retires Block's static unqualified/Default ClusterLabel.fill identities
(four solid/transparent matrix routes). Composite labels use NodeLabel styling;
the secondaryTextColor/tertiaryTextColor projection had no writer consumer. The
unused Block generic Text fallback is also removed. At that historical point, Class still
retained generic Text compatibility assignments; the later Class Text cutover retired them. Native pixel comparisons preserve
the composite label and retain an active Node paint control. Marker paint remains
legacy because its arrowMarkerPath consumer is real. Class edge-label background has an independent
historical and runtime witness: the legacy `.edgeLabel[data-look="neo"]` selector is present in
reference CSS but no emitted edge-label group carries that attribute, while actual label backgrounds
use `.labelBkg`/`.edgeLabel .label` selectors across Classic, Neo, HandDrawn, HTML-label, and SVG-label
runs. KTD23 v9 now retires Class's unqualified/Default EdgeLabelBackground.fill identities
(two historical identities, four solid/transparent matrix routes). The
[native pre-retirement baseline](../knowledge/engineering/verification/2026-09-13-class-background-baseline.md)
is fixed at `9babbfa248ea505053d24d8314b09d8e25a529b6`. After removal, all SVG bytes and decoded
PNG pixels match the unthemed scenes across five schemes, three looks, and both input label modes.
Unsupported background requests reconcile against completed visible label backgrounds; absent,
shadowed, and source-owned fills are NotApplicable. Missing label checkpoints stay incomplete.
This adds no typed background support. The later Edge scalar cutover leaves Class with 16 legacy routes. The
[retirement record](../knowledge/engineering/verification/2026-09-13-class-background-retirement.md)
separates current checks from the independent pre-removal baseline.

KTD23 v7 retires Block's two static unqualified/Default Title.fill identities
(four solid/transparent matrix routes). The pinned renderer has no diagram-title
terminal; frontmatter and composite node labels do not create one. Title requests
are Unsupported and NotApplicable in this empty domain. Native SVG/PNG comparisons
preserve existing visible labels, and no typed Title capability is claimed.

KTD23 v6 retires eight Flowchart/Swimlane static unqualified/Default Marker.fill/stroke
identities against baseline `395a4d2f202b6697f680bc386546f54b3ebcbebd`. Each formerly projected
only `marker.paint` to `themeVariables.arrowheadColor`. A pre-retirement Release investigation
covered 64 combinations across both families, both selectors and facets, solid/transparent, and
Classic, Neo, animated Neo, and HandDrawn. It confirmed that the old CSS selector
`.arrowheadPath` matches no emitted marker shape (`arrowMarkerPath`), all six actual references
resolve, and native PNG pixels equal the same scene without a Marker request. This investigation
is separate from the nonvisual KTD23 receipts and adds no typed Marker claim.

Flowchart and Swimlane now have no family bridge dispatch. Unsupported Marker rules are
reconciled over completed edge references, in start/end occurrence order. Unreferenced definitions
do not create occurrences. Source/config paint ownership suppresses only its own facet; missing
expected path completion remains incomplete. A winning request on an unowned visible marker
remains a residual, while absent, shadowed, or non-intersecting requests are NotApplicable.
The global provider remains necessary for Block's remaining generic Text and other legacy
routes. The earlier [Block NodeLabel cutover boundary](../knowledge/engineering/verification/2026-09-14-block-node-label-cutover-boundary.md)
records the shared-CSS design that preceded the typed replacement. NodeLabel now uses a
per-node HTML/SVG terminal plan, with source/config ownership and actual emitted-fragment
checks. The [terminal repair verification](../knowledge/engineering/verification/2026-09-14-block-node-label-terminal-repair.md)
records 3157 scoped Release tests and the executable SVG/PNG route authorization. Remaining
provider/probe retirement and the broader C7a delivery gates are separate work.

KTD23 v5 retires Sequence's four static unqualified/Default `Text.fill` and `Title.fill`
identities against baseline `096a8f7f3`. Both projected only `themeVariables.titleColor`, which
neither the pinned Mermaid 11.17.2 stylesheet nor the renderer consumes. The eight before/after
value probes authorize deletion without claiming a new typed text consumer. Three-look rendered
SVG comparisons preserve the actual title, actor, group, message, control, and note paint.

Each KTD17 route-profile witness binds SVG and PNG evidence; it is not a count of individual
output files. Version 80 adds twelve XY Chart Text.fill and Axis.fill/stroke static scalar
routes (solid/transparent, unqualified/Default), removing its final family bridge dispatch.
A shared paint plan preserves logical axis ownership in vertical and horizontal charts,
generic Text author order, and per-channel source/config precedence. Axis.fill supplies line
and tick paint only when Axis.stroke is unspecified. Final SVG receipts check title/axis
text, axis paths, and renderer-created rectangle labels; zero-size labels retain their
serialized appearance but cannot independently certify visible Text paint. Unsupported
winning sibling facets remain residual, while absent, source-owned, or shadowed requests
are NotApplicable. Independent native PNG comparisons isolate each text/axis channel in
both orientations. This cutover does not qualify ordinal Text or Axis paint.

Version 79 adds the four XY Chart Title.fill static scalar routes
(solid/transparent, unqualified/Default). The direct title plan preserves generic Text author
order and explicit `themeVariables.xyChart.titleColor` ownership. Evidence requires exactly one
matching title text and fill in the successfully finalized SVG; absent or hidden titles are
NotApplicable, and unsupported winning sibling facets retain residuals. XY Chart Text and Axis
remained legacy until version 80.

Version 78 adds the eight Quadrant Chart Axis.fill/stroke static scalar routes
(solid/transparent, unqualified/Default) and removes the final Quadrant family bridge dispatch.
A single paint plan owns text and border receipts so mixed rules require every winning facet.
Axis.fill retains its border fallback when Axis.stroke is unspecified; explicit stroke, transparent,
and Clear prevent fallback. Outer and inner border config ownership is independent of axis labels.
Border receipts cover paint, geometry, width, order, and plan identity. Unsupported Axis ordinal
requests are reconciled independently over label occurrences and the six border lines in writer
order; this does not qualify ordinal paint. Absent, source-owned, and shadowed requests are
NotApplicable. Quadrant captions retain their independent color channels.

Version 77 adds four Quadrant Chart Title.fill routes (solid/transparent,
unqualified/Default). The direct text plan now consumes dedicated title fill as well as inherited
Text fill, preserving author order and explicit title configuration ownership. Missing titles and
fully shadowed rules are NotApplicable; winning unsupported title facets remain residual.

Version 76 adds four Quadrant Chart Text.fill routes (solid/transparent,
unqualified/Default). The direct text plan preserves author-order inheritance into point labels,
axis labels, and the title; quadrant captions retain their separate color channels. Explicit
configuration owns each channel independently. Text ordinals follow points, axes, then title,
while Axis and Title selectors use their own occurrence domains. Winning unsupported or sibling
facets remain residual; absent, source-owned, or fully shadowed requests are NotApplicable.
The writer checks every text and fill against its prepared receipt before recording Applied.
Before version 78, the remaining Quadrant bridge handled only dedicated Axis paint. Axis.fill supplies
border paint only when Axis.stroke is unspecified; an explicit stroke, including transparent
or Clear, blocks that fallback. Generic Text.fill does not feed this border channel.

Version 75 adds eight Flowchart/Swimlane Edge.fill routes
(solid/transparent, unqualified/Default). Fill supplies the edge's native stroke only when Stroke
is unspecified; transparent, Clear, and unsupported Stroke winners all block that fallback.
The existing stroke writer and source/config precedence apply. Classic, Neo, and animated Neo
witnesses inspect native edge-stroke pixels. The line-color bridge and its implicit marker fallback
retire together; explicit Marker.fill/stroke remained the two families' only legacy routes
until KTD23 v6 retired their dead projections.
Version 74 adds eight Flowchart/Swimlane ClusterLabel.fill routes
(solid/transparent, unqualified/Default), with Classic, Neo, and HandDrawn native SVG/PNG
witnesses. ClusterLabel overrides only the properties it specifies on the existing Title/Text result;
author order within each role and the existing Title/Text winner semantics remain unchanged.
Explicit source/config owners retain precedence, including Base's title-color dependency.
Clear, ordinal, and unconsumed sibling requests retain residuals when they win at a visible terminal.
Overridden rules are NotApplicable. At that stage, the two families retained Edge.fill and
Marker.fill/stroke legacy routes.
Version 73 adds eight Flowchart/Swimlane EdgeLabelBackground.fill routes
(solid/transparent, unqualified/Default). Classic, Neo, and HandDrawn native witnesses prove
actual edge-label background paint. The Swimlane witness makes the lane fill transparent to
isolate the label's existing 50% opacity; it does not override the requested background color.
An explicit static request creates a sized Swimlane label background; unthemed output keeps its
existing sentinel. Sized labels with unsupported ordinal/sibling requests retain residuals even
when no background is emitted. Explicit config ownership and overwritten rules remain reconciled.
Version 72 adds eight Flowchart/Swimlane Text.fill routes (solid/transparent,
unqualified/Default). Classic, Neo, and HandDrawn witnesses bind the node-label, title, and
cluster-label projection obligations; additional PNG pairs isolate node, edge-label, cluster-title,
and diagram-title glyphs. Specific roles and explicit source/config owners retain precedence.
Ordinal and unconsumed sibling facets retain residuals. Version 71 adds four Radar Text.fill routes (solid/transparent, unqualified/Default).
Classic SVG/PNG witnesses cover inherited axis/legend text and the title fill fallback.
Radar no longer has a family bridge dispatch. Version 70 adds eight Radar Axis.fill/stroke static routes (solid/transparent,
unqualified/Default), with Classic SVG and native PNG evidence for axis-line stroke. The label
CSS `color` projection is preserved; it does not become a new label `fill` consumer. `lineColor`
and `axisColor` remain independently owned. Version 69 adds two Radar Title.fill Default routes.
Version 68 adds four Flowchart and four Swimlane Title.fill routes with Classic,
Neo, and HandDrawn witnesses for cluster and swimlane titles. Their typed classification suppresses
only those winners in the shared bridge. Version 72 also replaces Flowchart/Swimlane generic
Text fallback; Class retains its namespace-label title-color projection. Block's unused title-color projection is retired by KTD23 v7. Version 67 adds four Requirement Text.fill routes (unqualified/Default,
solid/transparent) with Classic, Neo, and HandDrawn witnesses. Separate native PNG pairs keep one
text-color owner fixed while proving the other node or SVG relation-label consumer. Version 66 adds
four Timeline Event.stroke routes (unqualified/Default, solid/transparent). Their Redux witnesses prove the visible activity axis with SVG and native
PNG; family receipts and SVG integration tests cover the shared node/label/line fanout. This does
not qualify shadow-bearing nodes for native export. Version 65 adds four Timeline Text.fill routes
(unqualified/Default, solid/transparent) with Classic, Neo and HandDrawn witnesses for inherited title and referenced
arrowhead fill. Version 63 adds eight Requirement Relation.fill/stroke routes (unqualified/Default,
solid/transparent), with Classic, Neo and HandDrawn witnesses for relation paths and referenced
markers. Explicit `relationColor` retains ownership; any specified stroke suppresses the fill
fallback. Version 62 adds the remaining eight ER scalar routes: unqualified Table.fill,
Default Text.fill, and unqualified/Default Relation.fill. Classic witnesses cover the row fills,
relation stroke channel, and visible text including the diagram title. Version 61 adds four ER
Table.fill routes (Odd/Even, solid/transparent) with Classic witnesses. Family tests also cover Neo/HandDrawn and HTML/SVG labels. Version 60 adds four GitGraph Node.stroke routes with Classic, Neo, and HandDrawn
witnesses covering tag outlines. Version 59 added four GitGraph Node.fill routes with Classic, Neo, and HandDrawn
witnesses covering state glyphs and tag backgrounds. Version 58 added four GitGraph Edge.fill routes with Classic, Neo, and HandDrawn
witnesses for branch-line stroke. Version 57 added eight GitGraph NodeLabel.fill/EdgeLabel.fill routes with Classic,
Neo, and HandDrawn witnesses covering visible tag and commit labels. Version 56 added four
GitGraph Text.fill routes with the same looks covering visible title, commit, and tag terminals. Version 55 added four Kanban Text.fill
routes with the same three looks. Version 54 added four Journey Text.fill routes with Classic
witnesses. The four C4 Text.fill routes retain Classic, Neo, and HandDrawn witnesses. Gantt Warning today and vertical
strokes retain separate profiles.
The today profile fixes the runtime clock to 2024-01-03 within the task interval; `todayMarker`
is a style directive, not a clock override. Neither terminal can supply the other's pixel evidence.
Neither KTD17 nor KTD23 is a percentage of all theme capabilities. Their selector/value domains
differ from the live bridge inventory and must not be added together to calculate migration progress.

ER Table.fill now has separate direct Odd and Even row consumers. Explicit `rowOdd` and
`rowEven` configuration owners suppress only their matching row route. Entity source fill owns
the even row path; odd row fill remains independent. Missing matching rows are NotApplicable.
Unqualified Table.fill directly consumes both row variants and retains both historical replacement
obligations. Unqualified/Default Relation.fill follows stroke-first fallback and uses the native
stroke channel. Unqualified/Default Text.fill covers visible labels, the diagram title, and SVG
subgraph labels when their color falls back from `titleColor` to `textColor`. Explicit configuration
and source styles retain ownership of each consumer. HTML labels consume `nodeTextColor`; SVG
labels and the diagram title consume `textColor`. Ownership is captured before theme-default
projection, so derived colors cannot become explicit owners. Title.fill remains Unsupported.
ER no longer requires its family bridge.

Radar static-unqualified and explicit `Default` `Title.fill` now share one direct typed writer and KTD17 SVG/PNG
authorization for solid and transparent values. Static unqualified/Default Axis.fill and Axis.stroke now use the same direct axis-line paint
consumer, with any specified stroke suppressing the fill fallback. Explicit `radar.axisColor`
owns the visible axis channel; emitting unused marker CSS cannot certify an application.
Text.fill now directly owns inherited axis/legend text and the unclaimed title fill fallback.
A Title.fill winner or explicit title-color owner blocks that fallback without taking ownership
of unrelated label text. Ordinal requests are reconciled over non-empty axis labels, visible legend
labels, then the non-empty title. Radar has no remaining family bridge routes or dispatch.

## Family Boundaries

`Typed` below describes a base-typography classification, not unconditional application. Explicit
source/config ownership, absent visible terminals, unsupported sibling properties, and output
qualification still affect the execution evidence. Role-local sizes remain independent where the
family writer owns them. A bridge-free family may deliberately support only a narrow typed surface.

| Family | Base FontStack | Base FontSize | Family-owned legacy bridge |
| --- | --- | --- | --- |
| State | Typed | Typed | None |
| Flowchart | Typed | Typed | None |
| Swimlane | Typed | Typed | None |
| Sequence | Typed | Typed | None |
| Class | Typed | Typed | Required |
| Block | Typed | Typed | Required |
| Mindmap | Typed | Unsupported | None |
| Tree View | Typed | Unsupported | None |
| GitGraph | Typed | Typed | None |
| Gantt | Typed | Unsupported | None |
| Kanban | Typed | Typed | None |
| Requirement | Typed | Typed | None |
| ER | Typed | Typed | None |
| Pie | Typed | Unsupported | None |
| XY Chart | Typed | Unsupported | Required |
| Radar | Typed | Typed | None |
| Quadrant Chart | Typed | Unsupported | None |
| Timeline | Typed | Typed | None |
| Journey | Typed | Typed | None |
| Architecture | Typed | Typed | None |
| C4 | Typed | Typed | None |
| Treemap | Typed | Unsupported | None |
| Packet | Typed | Unsupported | None |
| Sankey | Typed | Unsupported | None |
| Railroad | Typed | Typed | None |
| Info | Typed | Unsupported | None |
| Error | Typed | Unsupported | None |
| Cynefin | Typed | Unsupported | None |
| Wardley | Typed | Unsupported | None |
| Ishikawa | Typed | Typed | None |
| EventModeling | Typed | Typed | None |
| Venn | Typed | Unsupported | None |
| ZenUML | Unsupported | Unsupported | None |

For exact target/facet/selector coverage, inspect the family mechanism matrix and family-local
plans under `crates/merman-render/src/<family>/theme*`. Public discovery is independently declared
in the support manifest and reconciled against the matrix by tests; it does not imply Browser SVG
or native-export qualification for an otherwise unverified output.

Important boundaries that a family-level count cannot express:

- Journey static unqualified/Default Text.fill owns root inherited paint, native/HTML labels,
  actor legends, and generic line strokes. Explicit `themeVariables.textColor` remains the owner;
  `journey.titleColor` owns only the diagram title. Actor/task paints and the mouth's fixed stroke
  remain independent. Even a text-less diagram emits an activity line, so Text.fill is applicable.
  KTD17 replaces `text.fill`. KTD23 v3 independently retires the unused `title.fill` projection:
  `themeVariables.titleColor` styles only nonexistent cluster text. `Title.fill` is Unsupported;
  a visible title with no local color owner leaves a theme residual, while an absent title or
  explicit nonempty `journey.titleColor` is `NotApplicable`. No typed Title consumer is implied.
- GitGraph static unqualified/Default Text.fill owns root inherited paint, tag holes, visible
  titles, and Text-origin tag/commit label colors. KTD17 v56 replaces the inherited `text.fill`,
  `node-label.fill`, and `edge-label.fill` contributions together. KTD17 v57 directly migrates
  the eight static unqualified/Default solid/transparent NodeLabel.fill and EdgeLabel.fill routes,
  replacing only `node-label.fill` and `edge-label.fill`, respectively. Generic Text retains its
  three historical replacement obligations. Explicit `textColor`, `tagLabelColor`, and
  `commitLabelColor` remain independent owners; Text and label rules keep global source-order
  precedence. Color-generated commit text and branch colors retain their existing ownership. Missing inherited consumers are NotApplicable;
  unsupported winning siblings remain residuals. KTD17 v58 replaces Edge.fill's `edge.stroke`
  projection to `commitLineColor` with direct branch-line stroke, limited to static
  unqualified/Default solid and transparent paint. It preserves stroke-first fallback semantics
  and explicit `commitLineColor` ownership. An independent `lineColor` does not suppress this
  fill fallback; the existing direct Stroke route retains its source-selection policy. Hidden
  branches provide no terminal evidence. Unsupported ordinal combinations preserve static
  BestEffort CSS without certifying it as Applied. Arrows remain node-palette surfaces.
  KTD17 v59 migrates static unqualified/Default Node.fill, preserving the independent raw
  `primaryColor`, `mainBkg`, and `tagLabelBackground` owners captured before fallback projection.
  Materialized derived colors do not become explicit owners. The writer records the actual
  state glyph, tag background, and theme-dependent branch/highlight sources; absent consumers
  are NotApplicable. KTD17 v60 replaces the remaining Node.stroke projection. The same node paint
  plan owns `primaryBorderColor`, `nodeBorder`, and `tagLabelBorder` independently, including
  color-generated text/geometry and the gradient fallback where the writer actually consumes it.
  Existing gradient colors are not rederived, and unused CSS/defs do not establish application.
  GitGraph no longer has a family-owned legacy route or bridge dispatch.
  KTD23 gains no historical retirement rows.
- Kanban has no independent diagram-title terminal. KTD23 v4 retires the unused
  `title.fill` projection to `themeVariables.titleColor`; Title rules are Unsupported and
  NotApplicable, including documents with frontmatter titles. Column labels use Text paint and
  card labels use TaskLabel paint. KTD17 v55 replaces the Text compatibility projection and
  removes Kanban's final family bridge. Static unqualified/Default Text.fill updates the root and
  default label colors; explicit textColor, TaskLabel, and source inline colors retain ownership.
  With no inheriting visible labels Text.fill is NotApplicable; unverified source ownership and
  unsupported winning sibling properties remain residuals.
- C4 static unqualified/Default Text.fill owns the root inherited color and optional diagram title.
  Shape, boundary, and relationship labels retain independent colors. Explicit
  `themeVariables.textColor` owns the root; a title-less document with statically owned label colors
  is `NotApplicable`. Source-directed inheritance or browser-dependent colors remain unverified.
  KTD17 replaces `text.fill` and retires its unused `title.fill` fallback; this does not introduce
  a C4 `Title.fill` consumer.
- XY Chart shares a resolved `FontStack` across title/axis/legend measurement, layout, CSS, and
  visible-text receipts. Only base `FontSize` is Unsupported; role-local `xyChart.*` sizes remain.
- Class shares base typography with layout and final CSS; fixed-size cardinality terminals do not
  acquire the base FontSize. Source-owned descendant fonts require their own measurement evidence.
- Treemap shares its FontStack plan with text measurement and final output. Static unqualified and
  Default Text.fill preserve independent label/value config ownership; no participating text
  terminal means `NotApplicable`. Role-local sizes do not become a base FontSize consumer.
- Sequence no longer has a family bridge. Its generic Text.fill and Title.fill remain
  Unsupported: `titleColor` is not a consumer, while ActorLabel, MessageLabel, LoopLabel, and
  NoteLabel retain their independent typed paint. Successful terminal writers supply the text
  and title occurrence domains used to reconcile unsupported requests. A matching winner remains
  residual even when the unused `titleColor` config is explicit; absent, out-of-range, or shadowed
  rules are NotApplicable. This retirement adds no positive typography or text-color support.
- Requirement static unqualified/Default Text.fill directly replaces `requirement.text`.
  Node SVG/HTML labels and HTML relation labels consume `nodeTextColor`; SVG relation labels
  independently consume `relationLabelColor`. Source `color` declarations retain ownership.
  The retired `requirementTextColor` projection only addressed absent `.reqTitle`/`.reqLabel`
  terminals; the actual diagram title is outside this Text.fill surface. Receipts own the three
  active CSS declarations and reconcile prepared label identities with emitted label facts.
  Empty labels and self-loop anchors do not create ordinal occurrences. Matching unsupported
  ordinal rules and unknown dynamic colors remain residuals; overridden rules and absent or
  entirely source-owned consumers are NotApplicable. Requirement no longer has a family bridge.
- Timeline static unqualified/Default Event.fill is in KTD17, independently of its ordinal palette,
  radius, opacity, and typography routes. Static unqualified/Default Text.fill directly owns the
  root inherited paint; explicit textColor retains ownership. The writer records titles,
  referenced arrowheads, and node fill inheritance when the color-scale domain is empty.
  Independently colored node labels retain their owners, and dynamic source colors remain
  residuals. Unused marker definitions do not prove application. Static unqualified/Default
  Event.stroke directly owns Redux's shared `nodeBorder` paint: ordinary shapes, section labels,
  connectors, and the activity axis. Explicit `nodeBorder` retains ownership; Classic has no
  consumer. Redux Color and Neo shapes keep their independent border/gradient colors while
  labels and lines still consume the shared stroke. A title-only Redux diagram retains its
  activity-axis consumer. Writer-owned receipts bind the original CSS declarations to emitted
  terminals; KTD17 v66 separately proves the visible axis in SVG and PNG. Ordinal stroke remains
  Unsupported, with matching winners reported as residuals. Timeline has no family bridge;
  Title.fill remains Unsupported because Timeline has no titleColor consumer.
- Gantt Task[Warning].stroke is typed and in KTD17. Today-marker and vertical-marker source/config
  ownership remains terminal-local; no applicable marker is not evidence of typed application.
- Error-family bridge freedom does not erase unrelated detected-family compatibility from a lenient
  parse fallback. ZenUML's direct Title.fill does not imply generic text or typography support.

## Evidence and Release Boundary

| Stage | Evidence owned by the stage |
| --- | --- |
| Compile | Valid bounded recipe, fingerprint, declared/inferred capabilities |
| Family/document | Applicability, property winners, source/compatibility residuals, emitted terminals, prepared-text evidence |
| Export | Target-specific resource closure and SVG/PNG/JPEG/PDF admission |

An unevaluated stage cannot upgrade the result to portable. The private C6a ledger covers 18
representative Standalone SVG/PNG cells in nine Flowchart/State/Sequence render groups. Its
eligibility receipt requires that exact ledger; scalar cutovers and JPEG/PDF smoke do not add cells.
C6b equal-depth certification remains paused. Browser SVG qualification and the remaining C7a
authoring/consumer/preset gates are not complete. These counts do not establish stable readiness.

Route and raster-paint binding receipts are sealed by the opaque
`FamilyRenderCompletion<ResvgCompatibleSvg>` before the facade separates the output and report.
The former public seal functions accepting a report plus caller-provided artifact digests are
removed. The `internal-theme-acceptance` Cargo feature is also removed. The private harness uses
an explicit workspace cfg through `scripts/run_theme_acceptance.py`; producer Cargo packages
exclude its independent acceptance modules. `scripts/verify_theme_acceptance_boundary.py` checks
package lists and compiles a production consumer whose acceptance imports must fail. These
boundary checks do not qualify presets or close the remaining C7a authoring/consumer gates.

Support discovery now has an independent seventeen-case golden in
`crates/merman-theme-authoring-fixtures/fixtures/authoring-v1/support.json`. It pins manifest
revision 87 and checks V1/V2 rule, base-typography, and ordinal queries across Conditional,
Unsupported, NotApplicable, and Unverified states, including unknown identifiers and outputs that
are not renderer-qualified. Consumer tests share this fixture across the native C ABI, reusable and
one-shot UniFFI engines, Typst JSON, installed Node, installed Python, and both Web API/WASM
surfaces. Revision 82 added the retired Class background query; revision 83 adds the typed Block
background query. Revisions 84 and 85 add typed Class Edge.fill and Cluster fill/stroke queries.
Revision 86 added the typed Class namespace Title query; revision 87 adds Block Edge fill
and stroke-paint queries.
Installed N-API darwin-arm64, Node-WASM, and Python wheel artifacts built from clean source
`51f6308ac` pass all fifteen revision-86 support vectors: Node through asynchronous/synchronous
calls under pinned Node 24.13.1, and Python through one-shot/reusable APIs under both 3.12.8 and
3.9.6. Shared catalog, diagnostic, authoring, and installed Class namespace Title terminal checks
also passed; see the [revision-86 installed consumer record](../knowledge/engineering/verification/2026-09-13-installed-theme-consumers-revision86.md).
Revision-85 results remain historical. Browser Web packages, a new Typst WASM artifact, and
other host artifacts still need current-source verification. This is a cross-transport discovery
contract check; it does not qualify a document, preset, artifact profile, or C7a candidate.

The preset admission runner executes all ten exact catalog recipes against fixed Flowchart,
State, and Sequence sources, then exports each completed document to PNG. On 2026-09-11 the
60 artifact observations comprise 18 HostDependent Flowchart/State/Sequence outputs from the native
candidates and 42 Rejected outputs. The seven retained recipes keep explicit Mermaid `base` and `darkMode`
requests (two compatibility residuals per family). Native candidates omit those requests.
All recipes leave font resources to the host; standalone SVG fonts are not self-contained and
native text retains host font dependencies. Flowchart has no bridge residual in these fixed recipes;
EdgeLabelBackground.fill and generic Text.fill now have direct consumers. This admission change
does not expand the preset qualification scope.
Sequence's generic `Text.fill` fallback is now NotApplicable in this fixed source because completed
role-label writers cover every text fill. Missing role, title, or autonumber fill coverage and
non-fill facets retain their residuals; generic Text remains Unsupported when it would win.
Admission alone does not prove semantic or visual preset qualification. All catalog qualified scopes
remain empty; the scoped qualification runner covers declared Flowchart, State, and Sequence scenarios.

`inspect_preset_admission` binds the exact compiled recipe/resource fingerprints, source digests,
shared document/resource identities, target receipts, and actual artifact bytes. The admission
inventory is the first stage of preset qualification; it does not issue
`PresetQualificationReceipt`, assert semantic/visual qualification, or populate `qualified_cells`.

The separate `run_preset_qualification` runner now issues an opaque, execution-local receipt for
Brutalist, Spotless, and Cyberpunk on the declared `native-flowchart-state-sequence-system-fonts-v3` profile only.
Qualification schema 3 dispatches the exact catalog recipe through Flowchart/State/Sequence SVG
and PNG. Flowchart checks a titled subgraph containing two nodes and a labeled directed edge:
actual surfaces, borders, labels, marker and translucent background must reach SVG and PNG.
The edge label lies over the contrasting cluster fill so a missing background remains observable.
State checks canvas,
node surface, border and label assignments plus raster canvas/surface coverage and label ink.
Sequence checks actor, message, note, loop and autonumber labels, activation styles, and canvas
assignments through the sealed SVG observer; raster checks cover actor/note surfaces and label
ink, message strokes and visible lifeline strokes. State PNG uses 1x; Flowchart and Sequence
PNG use 4x to observe thin lines and label paint; records retain each scenario's scale. Lifeline sampling excludes observed actor,
note and activation rectangles and rejects fully occluded lines. Both target admissions
must remain HostDependent with only declared host-font reasons; unresolved fonts and any theme,
source or compatibility residual reject the run. No font or rule is injected. Negative tests cover
wrong-preset artifacts, blank output, missing ink, and recipe/resource/schema drift.

The [clean-build qualification runner](preset-qualification.md) now records the source commit,
lockfile, actual Release executable, toolchain/host, and complete scoped execution evidence. Its
freshness check rebuilds and reruns qualification and rejects any record mismatch. The optional
production CLI connection executes the exact sources and compares all qualified output bytes.
Release Preflight now applies this connection to the extracted, verified CLI archive, records its
binary/archive digests, and reexecutes both paths for record replay. This is host/CLI evidence;
it does not qualify other binding artifacts. Public qualified
scopes remain empty. Rust, binding JSON, Web, and Flutter now carry explicit `profile_id` and
`admission_status` alongside each cell's family/output IDs, preserving host-dependent and future
values without inferring portability. Generated catalog promotion still needs the profile's fresh
evidence connected to the actual artifact build. The C7a preset
gate remains open. These host-dependent scenarios do not qualify arbitrary sources, explicit
Marker/ClusterLabel rules, or a portable resource profile. Each native preset has six artifact cells
under this profile; these execution-local cells do not modify the fixed C6a ledger.

Preserve the seven retained recipes' compatibility behavior and the host's font policy. A test-only
resource profile cannot qualify the unmodified catalog recipe. Do not suppress these residuals to
make qualification pass. The fixed 18-cell C6a ledger remains unchanged.

The 2026-09-11 installed Python wheel consumer now runs shared light/dark definition and spec
oracles through generated UniFFI bindings. It verifies Flowchart/State/Sequence SVG reuse and
isolation, terminal State fill and a family-scoped override, complete-spec cold start, exact native
preset export/render equivalence, and explanatory Unsupported discovery. The existing wheel smoke
invokes this witness in Python CI, preflight, and final-wheel verification. The local run uses the
`python-uniffi-native` profile on aarch64-apple-darwin; other platforms retain their CI owners.
The default Python wheel has no PNG capability. This closes the missing executable non-Rust SVG
consumer witness, not C7a eligibility, contract freeze, or preset qualification.

The 2026-09-11 rebuilt Web/Playground text-surface smoke run passes all 21 cases in Chromium
and WebKit. Firefox passes 20 of 21: the Packet first byte label fails the existing 1px root-bound
containment assertion. A local Firefox probe reports the text element at x=6px against a root at
x=8px, while its DOM Range starts at x=9px; Chromium reports the element at x=9px. This is an open
browser geometry discrepancy, not proof that the glyph is clipped or permission to widen the
comparator tolerance. Keep the assertion and withhold cross-browser qualification until the
terminal geometry/pixel evidence resolves it. The production-mounted State dark Note case passes
in all three browsers after rebuilding the current transport artifacts and correcting the test's
duplicate share-fragment prefix.

Family paint adapters declare which targets require config-default ownership to be captured
after detection and before theme derivation. The shared compiler binds those declarations;
capturing ownership neither applies paint nor certifies a terminal. The legacy bridge separately
maps each family to its executable compiler or an explicit bridge-free classification. Its
dispatch inventory remains independent of the mechanism matrix.

KTD18 ordinal and KTD19 typography decisions remain in the convergence addendum, backed by
family tests and matrix classification. They do not yet constitute an independent combined
bridge-removal authorization. The current route/dispatch inventory is deliberately only an
inventory: it does not observe provider removal or close all release gates.

## Retirement and Verification Rules

- Do not delete the bridge while any executable Legacy route remains. Typed writers must preserve
  the intended behavior; an intentionally removed behavior needs an explicit Unsupported boundary
  and retirement evidence, not an empty overlay silently interpreted as success.
- Removing the final route also requires removing the compiler/parse provider and the historical
  probe dependency, then reconciling KTD17/KTD18/KTD19/KTD23 and support claims. Zero route counts
  alone do not authorize that deletion.
- Keep acceptance authority independent of runtime classification. Do not generate an authorization
  from the same inventory it is meant to check.
- Validate the final semantic terminal, not an arbitrary SVG substring. XML parse errors fail the
  test; unused CSS, defs, metadata, or absent terminals do not prove application.
- Preserve source/config precedence per property and occurrence. Raw theme CSS, custom
  postprocessors, host-dependent measurement, and browser behavior retain explicit evidence grades.
- Keep comparator normalization narrow and non-semantic; document residuals instead of hiding them.

Focused checks live in `merman-render` support-manifest/matrix/bridge unit tests and family SVG
tests, plus `merman-theme-acceptance` cutover and retirement tests. KTD17 library tests require
the `png` feature; running that crate with no default features alone does not exercise them.
Bridge inventory changes also require the `legacy_projection_retirement` integration target,
whose frozen counts and digests are independent of the renderer library tests:

```sh
python3 scripts/run_theme_acceptance.py nextest run --locked -p merman-theme-acceptance \
  --no-default-features --features png,layout-cytoscape \
  --test legacy_projection_retirement --test route_cutover_runtime
```

The semantic baseline is pinned by `docs/adr/0001-upstream-baseline.md` and
`tools/upstreams/REPOS.lock.json`. Config merge and precedence are documented in
`docs/alignment/CONFIG_FRONTMATTER_SUPPORT.md` and
`docs/alignment/MERMAID_THEME_STYLE_PRECEDENCE.md`. Historical implementation decisions remain in
`docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md`; this snapshot
supersedes its older progress counts, not its architectural or release requirements.
