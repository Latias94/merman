# Headless Parity Deepening - Handoff

Status: Active
Last updated: 2026-06-03

This workstream opens the post-11.15 structural-parity phase.

Current priority order:

1. HPD-080 visible rendering defect triage
2. HPD-050 Architecture-first layout engine audit
3. HPD-060 semantic/render unification pilot - done for Sequence
4. HPD-070 unsupported-family rubric - done

Immediate next task:

- HPD-010, HPD-020, HPD-030, HPD-040, HPD-060, and HPD-070 are done.
- HPD-080 is now the active priority override. Continue scanning for functional renderability
  failures that DOM parity can miss: blank output, hidden text, black labels/cards, missing semantic
  colors, and missing diagram-specific Mermaid 11.15 CSS/theme emission.
- HPD-050 remains an active residual-driven audit lane with multiple landed slices. Continue it only
  when there is a source-backed Architecture/Dagre/Graphlib seam to audit, not as a broad solver
  rewrite.
- Root residual work should wait behind HPD-080 when a supported diagram is visibly broken.

Current repository reality to preserve:

- Structural `parity` is green for the implemented matrix.
- `parity-root` remains the active numeric residual front, but visible rendering defects are higher
  priority than small root viewport tails.
- Honest top residual buckets are currently Flowchart `61`, Architecture `25`, Sequence `27`,
  Class `12`, Timeline `3`, Journey `2`.
- Sequence left-of wrapped note width semantics were improved in commit `cd9f02ff`, but a small
  root-width residual remains and should not be overfit without stronger evidence.
- Architecture remains the highest-value `manatee` / input-model audit target.
- This lane is not a license to drive every residual to zero with constants. Its purpose is to
  improve baseline truth, residual governance, and shared seams so later fixes are explainable.
- This lane also now treats renderability as an explicit quality gate: a DOM-parity SVG is not good
  enough if labels are invisible, cards/branch labels render as dark blocks, or Mermaid's semantic
  theme colors are missing.
- HPD-020 outcome to preserve:
  - `crates/merman-core/src/baseline.rs` owns the pinned Mermaid tag/version plus the explicit
    legacy generated suffix.
  - `Engine::default()` and live registry constructors now use
    `for_pinned_mermaid_baseline()` / `pinned_mermaid_baseline_*`.
  - xtask importers, bench entrypoints, and baseline report labels no longer present
    `default_mermaid_11_12_2*` as the active baseline truth.
  - Historical generated filenames still carry `11_12_2`; that is now explicit legacy provenance,
    not implied active baseline truth.
- HPD-030 outcome to preserve:
  - residual governance now uses six categories: source-backed behavior gap, generated measurement
    gap, browser lattice tail, stale baseline/override, solver/phase residual, and scope boundary.
  - Flowchart is currently dominated by browser lattice tails; Architecture is the main
    solver/phase front; Sequence and Class are the main generated-measurement fronts.
  - Counts remain queue-shaping hints only. They are not progress percentages and should not be
    used to justify fake completion claims.
  - Headless parity now has an explicit gate-tier policy: parser/semantic/error behavior,
    theme/CSS readability, structural DOM parity, and no blank/hidden/clipped/miscolored output are
    hard gates; `parity-root` numeric tails are diagnostic/regression sensors unless they are
    source-backed, visible/user-facing, stale, or explained by a reusable seam that survives
    family-level verification.
- HPD-040 outcome to preserve:
  - `svg_emitted_bounds` is now `svg/parity` infrastructure, not State-owned code.
  - Sequence note final wrap/measure logic is centralized and reused by layout, root-bounds, and
    SVG rendering.
  - No new override growth or ad hoc constants were introduced.
  - The Sequence long leftOf note root-width residual remains open (`570px` deterministic local vs.
    `566px` upstream; headless vendored report `585px` vs `566px`) and should not be overfit.
- HPD-050 in-progress outcome:
  - Architecture's FCoSE node `BoundsExtras` adapter is now a named helper
    (`architecture_fcose_node_bounds_extras`) with direct unit coverage.
  - The layout view no longer carries group title state, matching the current source-backed rule
    that group titles do not affect the pre-layout `eles.boundingBox()` relocation center.
  - The focused batch5 long-title residual stayed unchanged at upstream `542.926px` vs local
    `547.926px`; this pass was boundary cleanup, not a hidden root-width tune.
  - A second source-backed bounds slice fixed Architecture edge-label root bounds:
    `createText()` local y-range is now transformed for X/Y edge labels instead of being treated as
    a centered AABB, and compound label bottom now uses the source-backed `fontSize + 1px` rule.
  - This made `stress_architecture_batch4_init_small_icons_061`,
    `stress_architecture_batch4_init_fontsize_wrap_063`, and
    `stress_architecture_edge_label_corner_cases_012` root-green without adding root overrides.
  - Full Architecture structural parity stayed green; Architecture `parity-root` had `26`
    mismatches at that point. The remaining top tails were `junction_fork_join_026` (`+13.976px`),
    `batch5_long_titles_and_punct_076` (`+5.000px`), and `html_titles_and_escapes_041`
    (`+5.000px`).
  - A follow-up `junction_fork_join_026` audit found no new source-input mismatch, but corrected
    the earlier baseline-drift reading. Current local service positions match the saved Mermaid
    old debug probe `target/compare/arch_junction_fork_join_probe_m15rv089.json` to floating-point
    noise; however, a fresh Edge-backed `check-upstream-svgs` run reproduces the stored upstream
    fixture exactly. Treat this row as a probe-harness / CLI-harness divergence plus solver/phase
    residual candidate; do not tune manatee against the saved debug probe alone.
  - A follow-up bounds-seam cleanup removed the unused renderer-side `initial_center` / pre-layout
    group bbox model and renamed the old generic compound padding helper to
    `architecture_svg_group_bbox_padding_px(...)`. The remaining `batch5_long_titles` and
    `html_titles` `+5px` tails are confirmed group/service Cytoscape bbox measurement residuals;
    do not close them by globally removing the final SVG group bbox extra.
  - A probe-harness correction updated
    `tools/debug/arch_fcose_browser_probe_fixture_025.js` to be explicit that it is a manual
    diagnostic reconstruction, not an authoritative Mermaid CLI render. It now mirrors xtask's
    deterministic page prelude more closely and reads shipped Architecture FCoSE config fields, but
    it still does not reproduce the stored CLI fixture for `junction_fork_join_026`. Important:
    the installed `mermaid@11.15.0` dist used by `tools/mermaid-cli` does not contain the later
    repo-ref `withSeededRandom` Architecture seed path, so do not change `manatee` to that
    `mulberry32` behavior unless the baseline package changes.
  - The local `repo-ref/mermaid` checkout is restored to the pinned Mermaid `11.15.0` commit
    `41646dfd43ac83f001b03c70605feb036afae46d` in detached-HEAD state, matching
    `tools/upstreams/REPOS.lock.json`. For any source-backed HPD-050 claim, use that pinned
    checkout, the installed `tools/mermaid-cli` dist, or fresh `check-upstream-svgs` output.
  - A follow-up Cytoscape bbox phase audit enhanced the debug probe to expose pre-layout
    `labelWidth`, `labelHeight`, `labelBounds`, `bodyBounds`, `autoWidth`, `autoHeight`, and
    `autoPadding`. For `batch6_init_fontsize_icon_size_wrap_093`, the probe explains the remaining
    `-2.5px` row: leaf service `node.boundingBox()` differs from the child contribution used by
    `updateCompoundBounds()`. An exploratory global production formula made that row exact but
    expanded the full Architecture root mismatch count from `26` to `47`, so it was rejected and
    reverted. The next real fix needs a phase-specific bbox model rather than a single global group
    padding/label-width rule.
  - The first safe follow-up to that finding is now landed: Architecture service bounds estimate
    fields are named by phase (`emitted_icon_bounds`, `svg_root_bounds`,
    `cytoscape_group_child_bounds`). This did not change behavior: structural Architecture parity
    was green, and `parity-root` remained the expected 26 mismatches before the later isolated
    service follow-up.
  - `repo-ref/dagre` and `repo-ref/graphlib` are now present and checked out to the pinned
    lockfile commits, so dugong/graphlib source-backed audits no longer have to proceed from stale
    assumptions.
  - `dugong-graphlib` now has an explicit upstream coverage ledger in
    `docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md`. The first direct Graphlib source-test slice
    ports the exposed helper algorithms (`components`, `findCycles`, `preorder`, `postorder`) and
    tightens missing-root traversal behavior to panic like upstream Graphlib throws.
  - `tools/dagre-harness/run.mjs` now imports `dagre-d3-es` from the installed
    `tools/mermaid-cli/node_modules` baseline and is executable again. A focused State `basic`
    Dagre layout comparison reported zero node and edge delta.
  - The next dugong-adjacent audit should target the public Graphlib `Graph` API subset used by
    `dugong` and Mermaid-facing renderers. Do not spend HPD-050 budget implementing unused
    shortest-path algorithms unless a real Mermaid/Dagre path needs them.
  - That public Graphlib `Graph` API audit has started. The first `graph-test.js` slice now covers
    options, labels, node/edge basics, named multiedges, compound parent moves, root children, and
    remove-node cleanup. `set_parent_ix(...)` now enforces the upstream tree invariant by panicking
    if a parent assignment would create a cycle. The non-compound `setParent(...)` throw remains an
    open Rust API-shape decision.
  - The next public Graph API slice covers source-backed edge/adjacency queries: `sinks`,
    predecessor/successor/neighbor queries, `isLeaf`, `inEdges`, `outEdges`, `nodeEdges`, and
    remove-edge neighbor count behavior. New Rust API seams are limited to `sinks`, `is_leaf`, and
    `node_edges_between`, because those map to existing Graphlib behavior and real graph consumers.
    Missing-node `undefined` returns and JS chainable mutators remain documented Rust/JS API-shape
    differences, not hidden parity claims.
  - A follow-up Graphlib edge-invariant slice now matches upstream's non-multigraph named-edge
    guard: named edge insertion panics on simple graphs, and named lookup/removal does not alias the
    unnamed edge. Renderer graph construction that uses edge names is already multigraph-based, so
    this should be treated as a source-backed invariant fix rather than a rendering tune.
  - The next Graphlib public API slice is also landed: `filter_nodes(...)` now copies selected
    nodes/edges/options/graph label and applies Graphlib's compound parent promotion when an
    intermediate parent is filtered out. Default node and edge label callbacks can now receive the
    node id or edge endpoints/name through explicit Rust API methods while the existing no-arg
    setters remain available. These APIs are method-scoped parity seams; they do not add broad
    Clone bounds to ordinary layout graphs.
  - A follow-up Graphlib children/root slice is now landed. `children_opt(...)` exposes the
    source-backed optional query shape for missing nodes versus existing nodes with no children,
    while `children_root()` remains the Rust mapping for Graphlib's no-argument `children()` root
    query. The existing `children(...) -> Vec<&str>` behavior is intentionally unchanged for Rust
    callers.
  - A follow-up Graphlib `setPath(nodes, value)` slice is now landed. `set_path_with_label(...)`
    sets and updates the same edge label across every path edge with only a method-scoped
    `E: Clone` bound.
  - A follow-up Graphlib `setNodes(nodes, value)` slice is now landed. `set_nodes(...)` uses
    default node labels without changing existing node labels, and `set_nodes_with_label(...)`
    batch-applies one node label with only a method-scoped `N: Clone` bound.
  - A follow-up Graphlib parent/clear-parent coverage slice is now landed. It maps Graphlib's
    `parent(v)` optional query shape and `setParent(v)` clear-parent state behavior to existing
    Rust `parent(...)` and `clear_parent(...)` APIs without adding JS optional-argument overloading.
  - A follow-up Graphlib `setEdge` optional-label / EdgeKey coverage slice is now landed.
    It maps explicit JS `undefined` edge-label clearing to `Option<T>` edge labels and maps
    Graphlib edge-object parameters to the existing Rust `EdgeKey` API.
  - ARCH-022's first Dagre reference adapter slice is now landed. The Rust-side input schema,
    Rust/JS output comparison, JS harness invocation, and compound-edge normalization now live in
    `crates/xtask/src/cmd/debug/dagre_reference.rs`; `compare-dagre-layout` remains State-only and
    acts as a graph producer/command wrapper. Basic, composite, and internal-cluster State Dagre
    comparisons all reported zero node and edge delta after the extraction. Do not broaden this to
    other diagrams until a real Dagre-backed residual audit needs that producer.
  - Architecture Cytoscape service-label measurement now has a shared
    `ArchitectureCytoscapeServiceLabelExtension` seam used by both FCoSE node `BoundsExtras` and
    SVG root/group service-bounds estimation. This reduces hidden duplicate measurement logic while
    preserving the then-known 26 Architecture root residuals; SVG root `createText(...)`
    measurement remains separate from Cytoscape compound-child label measurement.
  - A follow-up disconnected-islands root-bounds audit confirmed why that phase split matters.
    `stress_architecture_disconnected_islands_046` was width-aligned but height-off before the
    follow-up fix: local final root was `823.346x775.647` versus upstream `823.346x768.460`. The
    emitted SVG scanner alone was too short (`823.346x751.460`), while the final root became too
    tall after unioning synthetic label `content_bounds`. A temporary top-level-service switch from
    `svg_root_bounds` to
    `cytoscape_group_child_bounds` fixed this one row but expanded full Architecture root mismatches
    from `26` to `84`, so it was rejected.
  - The first narrow phase-specific follow-up is now landed:
    `architecture_top_level_service_root_bounds(...)` uses `cytoscape_group_child_bounds` only for
    isolated top-level services in diagrams that also have groups. The disconnected-islands row is
    now exact at `823.346x768.460`, full Architecture structural parity is still green, and the
    Architecture root mismatch count is down to `25`. Do not broaden this into a global
    service-bounds switch.
  - Fresh 2026-06-03 classification confirms the Architecture family structural report is green
    and the root-only report has `25` mismatches. The next residual audits should start from
    the `+5px` group/service bbox rows (`batch5_long_titles_and_punct_076`,
    `html_titles_and_escapes_041`) and any fresh report regression. The
    `junction_fork_join_026`, `unicode_and_xml_escapes_019` / `nested_groups_002`
    compound-bounds class, and `group_port_edges_017` are classified below. Do not reopen
    `batch4_init_small_icons_061`,
    `batch4_init_fontsize_wrap_063`, `edge_label_corner_cases_012`, `fan_in_out_021`,
    `deep_nesting_013`, `batch6_junctions_multi_split_with_group_edges_087`, or
    `disconnected_islands_046` unless a fresh report regresses.
  - A follow-up group-bbox phase audit rechecked the two `+5px` rows on current HEAD. Both are
    still controlled by final group rect width, not service placement or root-finalize CSS. A
    temporary experiment removing `ARCHITECTURE_SVG_GROUP_BBOX_EXTRA_PADDING_PX=2.5` made those two
    rows width-exact but height-short and regressed many group-heavy rows, so it was reverted before
    commit. Continue toward a phase-specific Cytoscape bbox model; do not globally remove the final
    group bbox extra.
  - A source-formula follow-up split those two `+5px` rows more precisely. Browser finalElements
    show `pipeline.autoWidth=379.926` / `node.boundingBox().w=462.926` and
    `ui.autoWidth=316.926` / `node.boundingBox().w=399.926`; local group debug shows content widths
    `382.926` and `319.926`, then final widths `467.926` and `404.926`. The row shape is therefore
    roughly `+3px` child-contribution drift plus `+2px` final group formula drift. Two tempting
    experiments were rejected: split-axis group padding made the focused rows green but reopened
    many group-heavy rows, and standalone final group extra `+1.5` only improved them to `+3px`
    while still reopening many rows. The next real implementation should model Cytoscape
    `children.boundingBox({ includeLabels: true, includeOverlays: false })` before applying the
    final group `outerWidth + body expansion` formula.
  - A children-bbox probe follow-up now records parent
    `childrenBoundingBoxIncludeLabels` and `childrenBoundingBoxBodyOnly` directly. For the two
    focused `+5px` rows, `childrenBoundingBoxIncludeLabels.w` exactly equals browser `autoWidth`
    (`pipeline=379.926`, `ui=316.926`), while `childrenBoundingBoxBodyOnly.w=282.926`. Browser
    service label bounds follow Cytoscape's `labelWidth + 4` rule, but current Rust child-label
    contributions differ by a non-uniform pattern (`+1`, `+2`, `+4`, `0`, `+2`, `-8`, `+4` across
    the sampled labels). Do not apply a uniform subtract-N, global label-scale, or group-padding
    patch for these rows; the next safe implementation needs a phase-specific service
    labelBounds/bodyBounds union helper and full Architecture root verification.
  - A follow-up source-phase experiment tested that helper direction. LabelBounds-only source
    formula improved the two `+5px` rows and reduced Architecture root mismatches from `25` to
    `24`, but it was rejected as a half-source model that worsened already-small rows. The fuller
    source phase model (child body `+1px`, child labelBounds `ceil(width)/2 + 2`, final group
    padding `padding + 1.5`) improved the focused rows to `+2.5` and `+1.5` and kept structural
    Architecture parity green, but expanded full Architecture root mismatches from `25` to `100`.
    Both production experiments were reverted. Treat this as evidence that `parity-root` is a
    diagnostic sensor for Architecture browser tails, not a mandate to directly import raw
    Cytoscape source phases before the headless measurement model is strong enough.
  - The Architecture browser probe now emits `finalElements` after the second FCoSE run. Use it to
    read final `node.boundingBox()`, `labelBounds`, and `bodyBounds` directly instead of inferring
    final group bboxes from SVG rects. It has been checked on the `unicode_and_xml_escapes_019` and
    `batch5_long_titles_and_punct_076` residual rows.
  - A follow-up `junction_fork_join_026` finalElements and FCoSE input audit reconfirmed that the
    stored upstream fixture is reproducible and the manual probe still does not reproduce the CLI
    fixture. Browser/Rust constraints match pinned Mermaid source: junction parents come from
    `junction.in`, duplicate relative-placement rows are preserved, and same/cross-parent edge
    ideal lengths/elasticities match the source callbacks. Keep this row classified as
    source-input-matched manatee-vs-Cytoscape FCoSE solution/internal phase residual. The next real
    move would be a `cytoscape-fcose` / `cose-base` reference harness, not renderer or root tuning.
  - A follow-up `group_port_edges_017` finalElements audit reconfirmed this is not a group-edge
    shift or SVG path emission bug. Local X spread is about `1.468px` wider, and local Y spacing is
    about `17.845px` more compressed than the browser/Cytoscape result, matching the root deltas.
    Treat it as manatee-vs-Cytoscape FCoSE solution / compound-bound drift unless a reusable
    source-backed solver rule appears.
  - A follow-up `batch6_init_fontsize_icon_size_wrap_093` finalElements audit reconfirmed this is
    not a root-finalize or height issue. Browser final group bboxes are `left.w=162,h=124` and
    `right.w=236.605,h=160.924`, while local group rects are `left.w=159,h=124` and
    `right.w=235.605,h=160.924`. The remaining `-2.5px` root width tail is a custom-init
    service/group child bbox phase residual; the earlier global formula that made it exact expanded
    full Architecture root mismatches from `26` to `47`, so keep it classified until a reusable
    phase-specific bbox model exists.
  - A follow-up `nested_groups_002` finalElements audit reconfirmed this is not an SVG group-rect
    translation, configured-padding, root-finalize, or edge-emission issue. Pinned Mermaid source
    renders group rects directly from final `node.boundingBox()` plus `iconSize/2`, and the upstream
    SVG matches that. Local services are uniformly shifted about `+1.25px` on X, while nested
    `data` and `platform` group widths are `0.5px` short, producing the `+2.5px` root tail after
    propagation. Keep it classified as nested compound-bounds phase residual.
  - A follow-up `unicode_and_xml_escapes_019` finalElements audit reconfirmed this is not parser
    escaping, label decode, SVG group-rect translation, root-finalize, or edge-emission drift. The
    stored upstream and local SVGs emit the same decoded label words. Browser group `i` final bbox
    is `389.822x383.593`, while local emits `392.822x383.593`; local service positions are about
    `-1.5px` on X with matching Y. Treat it as service label / group child bbox phase residual.
- HPD-060 outcome to preserve:
  - Sequence now uses the typed `SequenceDiagramRenderModel` as the semantic source for
    compatibility JSON projection.
  - `SequenceDb::into_model(...)` delegates through `into_render_model().to_compat_json(...)`
    instead of maintaining a second manual JSON construction path.
  - The focused parse test covers actor order, messages, notes, boxes, create/destroy indexes, and
    omitted optional message fields (`placement`, `centralConnection`).
  - Sequence structural SVG parity stayed green after the change. Sequence root parity still has the
    existing measurement residual front (`28` dom mismatches in the focused post-HPD-060 report);
    do not present this pilot as root-bounds closure.
- HPD-070 outcome to preserve:
  - `docs/alignment/UNSUPPORTED_FAMILY_ADMISSION_RUBRIC.md` owns the unsupported-family admission
    gates and priority order.
  - Priority order for pinned Mermaid 11.15 unsupported families is `treeView`, `ishikawa`,
    `eventmodeling`, `venn`, then `wardley`.
  - `venn` must not be implemented with a guessed circle layout; it needs a source-backed
    `@upsetjs/venn.js` layout plan.
  - `railroad-*` and `cynefin-beta` are absent from the pinned Mermaid 11.15 source, even if a newer
    `repo-ref/mermaid` checkout later contains them. Do not include them in the 11.15 parity backlog
    unless the baseline is bumped.
- HPD-080 in-progress outcome:
  - The first source-backed CSS/theme slice fixed the defect class exposed by the user's Kanban and
    GitGraph examples: structurally valid SVGs that were unreadable because diagram-specific
    Mermaid 11.15 styles were missing or incomplete.
  - Kanban emits source-backed section/ticket/icon/label theme CSS. Packet maps `packet.*`
    `PacketStyleOptions`. Sankey emits config-aware label/link style rules. C4 emits config-aware
    base CSS and `.person` theme colors. GitGraph emits classic/default per-branch theme rules for
    branch labels, commits, arrows, labels, merge/reverse commits, and highlight inner colors.
  - The user GitGraph merge sample now renders readable branch labels and colored branch/merge
    paths; the manual PNG evidence is `target/compare/gitgraph_user_merge.png`.
  - A second HPD-080 slice fixed the same class for Gantt, Treemap, and Requirement. Gantt now
    reads theme variables and emits outside done/doneCrit contrast rules; Treemap maps `treemap.*`
    style options plus theme title/text colors; Requirement maps requirement/relationship/label
    theme colors instead of using stale fixed colors.
  - A third HPD-080 slice fixed Mindmap theme CSS emission. Mindmap section/root colors now read
    Mermaid 11.15 `THEME_COLOR_LIMIT`, `cScale*`, `cScaleLabel*`, `cScaleInv*`, `git0`,
    `gitBranchLabel0`, `nodeBorder`, `theme`, and `look`; local XHTML labels now receive section
    `span` colors instead of the old single `.section-2 span` fallback. Upstream `data-look`
    gradient/drop-shadow rules remain intentionally un-emitted until local SVG nodes actually emit
    those attributes.
  - A fourth HPD-080 slice fixed Pie theme CSS emission. Pie now passes `effective_config` into
    CSS generation and reads Mermaid 11.15 `pie*` theme variables for stroke, opacity, title,
    slice-label, and legend text styles. The obsolete fixed `info_css(...)` wrapper was removed.
  - A fifth HPD-080 slice fixed Journey theme CSS emission. Journey now reads source-backed
    `faceColor`, `mainBkg`, `nodeBorder`, `arrowheadColor`, `edgeLabelBackground`, `titleColor`,
    `tertiaryColor`, `border2`, `fillType0..7`, and optional `actor0..5` variables; this matters
    because task/section CSS classes override SVG `fill` presentation attributes.
  - A sixth HPD-080 slice fixed ER theme CSS emission. ER now reads source-backed entity, label,
    relationship line, marker, edge-label, error, and `look: neo` stroke variables instead of stale
    default-theme colors. Upstream ER `data-color-id` and neo label-background rules remain
    intentionally un-emitted where local SVG elements do not carry the required attributes.
  - A seventh HPD-080 slice fixed Radar style override emission. Radar CSS now resolves top-level
    `radar.*` style options before `themeVariables.radar.*`, matching Mermaid 11.15's
    `cleanAndMerge(themeVariables.radar, radar)` behavior for axis, graticule, curve, and legend
    styling.
  - An eighth HPD-080 slice fixed Block composite cluster theme CSS. Nested block clusters now use
    source-backed `fade(clusterBkg, 0.5)` and `fade(clusterBorder, 0.2)` semantics where colors are
    parseable, preserving the configured color only for unresolved runtime CSS expressions.
  - A ninth HPD-080 slice fixed Sequence theme CSS emission. Sequence now passes
    `effective_config` into CSS generation and reads Mermaid 11.15 actor, lifeline, signal, label,
    loop/section, note, activation, root text, marker/error, node-border, drop-shadow, and optional
    `noteFontWeight` theme variables through `SvgTheme`. Upstream neo-only `data-look` /
    `outer-path` selectors remain intentionally un-emitted until local Sequence SVG nodes carry
    those attributes.
  - A tenth HPD-080 slice fixed State theme CSS emission. State now reads Mermaid 11.15 state node,
    cluster, transition, label, note, marker, start/end, special-state, and title theme variables
    through `SvgTheme`; the prefixed local barbEnd marker now uses source-backed suffix selectors,
    while dependency marker and neo gradient/drop-shadow rules remain intentionally un-emitted where
    local SVG output/defs do not support them.
  - An eleventh HPD-080 slice fixed Flowchart stroke-width theme CSS. Flowchart now reads
    Mermaid 11.15 `themeVariables.strokeWidth` through `SvgTheme::css_value(...)` for visible node
    and edge-path stroke widths instead of hardcoding `1px` / `2.0px`; Flowchart structural parity
    stayed green.
  - A twelfth HPD-080 slice fixed the adjacent Class namespace structural issue. Core Class parsing
    now preserves Mermaid 11.15 namespace-qualified relation facade classes/endpoints instead of
    resolving them back to namespace members; ASCII rendering keeps concise output by folding only
    empty namespace facade boxes back to declared members as a view-layer alias.
  - A thirteenth HPD-080 slice fixed Timeline `.disabled` theme CSS. Timeline disabled node/text
    fills now read Mermaid 11.15 `themeVariables.tertiaryColor` and `clusterBorder` instead of
    hardcoded fallback colors; redux/neo gradient/drop-shadow rules remain deferred where local SVG
    does not emit the required support attributes/defs.
  - A fourteenth HPD-080 slice fixed Architecture theme CSS emission. Architecture edge/group CSS
    now reads Mermaid 11.15 `archEdgeColor`, `archEdgeArrowColor`, `archEdgeWidth`,
    `archGroupBorderColor`, and `archGroupBorderWidth` instead of falling back to generic
    `lineColor`, `primaryBorderColor`, and hardcoded widths. This is a visible style fix only; it
    does not alter Architecture layout/root residuals.
  - A fifteenth HPD-080 slice fixed Class note theme colors. Class note shapes now use Mermaid
    11.15 `noteBkgColor` and `noteBorderColor` for both HTML-label and `htmlLabels:false` render
    paths, while existing Class CSS continues to drive `noteTextColor`.
  - A sixteenth HPD-080 slice fixed remaining source-backed Class stylesheet coverage that applies
    to current output. Class CSS now emits Mermaid 11.15 node shape, divider, cluster,
    class-label, edge-terminal, and relation rules from `class/styles.js`, and reads numeric
    `themeVariables.strokeWidth` through `SvgTheme::css_value(...)` instead of dropping it through
    a string-only color lookup. Icon and neo-only rules remain intentionally deferred where local
    Class output does not emit the required support attributes/elements.
  - A seventeenth HPD-080 slice audited Zed PR 57967's 0.6 integration feedback. Zed's color
    cleanup remains a host theme override concern, but its fallback de-duplication fix exposed a
    general `resvg_safe` integration need. `DropNativeDuplicateFallbacksPostprocessor` is now a
    public optional pipeline pass: it keeps fallback-only labels but drops fallback groups whose text
    duplicates native SVG `<text>`, matching the safer behavior Zed had to implement locally without
    changing the default `resvg_safe()` contract.
  - An eighteenth HPD-080 slice added
    `docs/workstreams/headless-parity-deepening/THEME_RENDERING_COVERAGE.md`, a source-backed ledger
    for Mermaid 11.15 style-provider coverage, inline-only diagrams, deferred inert rules, and host
    theme integration boundaries. It explicitly records that Zed-like palette cleanup is host policy,
    while fallback markers/de-duplication are merman integration contracts.
  - A nineteenth HPD-080 slice added XYChart inline theme render-path coverage. Mermaid 11.15 has no
    XYChart style provider, so the correct parity shape is not invented CSS; the new test proves
    `themeVariables.xyChart` reaches chart background, title, axis labels/titles, ticks, axis lines,
    and bar/line plot palette colors in final SVG output.
  - A twentieth HPD-080 slice added a public API dark-theme renderability smoke for Flowchart,
    Sequence, Kanban, GitGraph, and XYChart. It found and fixed a real Flowchart CSS omission:
    Mermaid 11.15 labels use `nodeTextColor || textColor`, and local Flowchart now emits that rule.
    The same smoke was calibrated against pinned Kanban source/fixtures so upstream
    `class="node undefined"` / `class="cluster undefined ..."` placeholders and priority side-line
    rendering are not treated as local defects.
  - A twenty-first HPD-080 slice fixed QuadrantChart's invalid default data-point color. Pinned
    Mermaid 11.15 intends `quadrantPointFill` to be a lightened/darkened `quadrant1Fill`, but calls
    khroma `lighten` / `darken` without the required amount argument and emits
    `hsl(...NaN%)`. Local headless output now derives a valid 10% lightness-shift default while
    preserving valid explicit `quadrantPointFill` overrides. xtask DOM parity normalization treats
    only this known QuadrantChart default point-color slot as an upstream invalid-token artifact;
    strict DOM comparison still exposes the real difference.
  - A twenty-second HPD-080 slice removed useless invalid inline style artifacts from ER and
    Mindmap edge paths. Upstream fixtures contain `style="undefined;;;undefined"`, but the visible
    edge behavior is class-driven and the token is not a meaningful style contract. Local raw SVG
    no longer leaks that string while ER and Mindmap structural parity remain green.
  - A twenty-third HPD-080 slice corrected the Mermaid 11.15 public theme surface after rechecking
    the upstream theme registry and config types. Core, bindings, and `@merman/web` now expose all
    11 official theme names, including `neo`, `neo-dark`, `redux`, `redux-dark`, `redux-color`, and
    `redux-dark-color`. Extended theme defaults use the generated 11.15 snapshot; later HPD-080
    work added a visible derived-key override seam without claiming a full hand-port of every
    extended theme rule.
  - A twenty-fourth HPD-080 slice exposed the generic duplicate-fallback cleanup through shared
    binding `options_json` as `svg.drop_native_duplicate_fallbacks`. The default remains unchanged;
    non-Rust hosts can now opt into the same safe duplicate native/fallback label cleanup that Rust
    users had through `DropNativeDuplicateFallbacksPostprocessor`.
  - A twenty-fifth HPD-080 slice re-audited Zed PR 57967 against the current 0.7 theme surface.
    Rust host theme workflows are covered through site config, `themeCSS`, scoped CSS
    postprocessors, and raster-safe pipelines. Bindings cover raster-safe output and fallback
    de-duplication, but still lacked first-class external `site_config` or host-scoped CSS options.
    Treat host palette cleanup as API ergonomics work; do not turn Zed's editor palette cleanup into
    default Mermaid output.
  - A twenty-sixth HPD-080 slice closed the Mermaid-config half of that binding gap:
    `options_json.site_config` now accepts a Mermaid config object, validates that it is an object,
    and feeds `HeadlessRenderer::with_site_config(...)` / `HeadlessAsciiRenderer::with_site_config(...)`.
    Non-Rust hosts can now pass official themes, `themeVariables`, diagram config, and Mermaid
    `themeCSS` without injecting directives into the diagram source. At that point, host-owned
    palette CSS was still manual or Rust-postprocessor-only pending a security/cascade/raster-safety
    design.
  - A twenty-seventh HPD-080 slice closed that binding host-CSS gap with an explicit host-owned API:
    `svg.scoped_css` maps to `ScopedCssPostprocessor`, `svg.css_override_policy` controls whether
    existing `!important` flags are preserved or stripped, and `resvg-safe` binding pipelines run
    `SanitizeCssPostprocessor` after host CSS injection. This is for host-provided CSS only; it
    still does not add Zed-specific default palette behavior or root background stripping.
  - A twenty-eighth HPD-080 slice reconciled the root white-background boundary. Pinned Mermaid
    11.15 `setupGraphViewbox` emits root `max-width` but not `background-color`; our stored upstream
    baselines get white backgrounds from the capture path, and local parity SVG preserves that
    shape. Rust hosts can now use `RootBackgroundPostprocessor`, while binding hosts can pass
    `svg.root_background_color` to rewrite only the root canvas color. Defaults stay unchanged.
  - A twenty-ninth HPD-080 slice re-audited common host theme needs against Zed PR 57967 and the
    current 0.7 API. Common flows are covered by site config, `themeCSS`, scoped host CSS,
    `resvg-safe`, optional duplicate fallback cleanup, and root background replacement. Advanced
    element/inline-style palette rewriting remains a host-owned postprocessor boundary, not a
    default merman theme behavior.
  - A thirtieth HPD-080 slice expanded the public API dark-theme renderability smoke to cover
    Class, State, Architecture, Block, Journey, Radar, Requirement, Timeline, Gantt, Treemap, and
    Pie in addition to the earlier Flowchart, Sequence, Kanban, GitGraph, QuadrantChart, and
    XYChart cases. Timeline's `node-bkg node-undefined` class is narrowly allowed because pinned
    upstream Timeline SVG fixtures emit that placeholder shape too.
  - A thirty-first HPD-080 slice filled the remaining compact public-smoke coverage gap for ER,
    Mindmap, C4, Packet, and Sankey. No production fix was needed; the slice proves these diagrams'
    labels and source-backed theme/config colors survive the public `HeadlessRenderer` route. C4 is
    intentionally asserted through visible C4 config colors, while Info/Error/ZenUML remain
    boundary cases unless a concrete visible failure appears.
  - A thirty-second HPD-080 source-checkout audit found that `repo-ref/mermaid` had drifted to
    `develop`, which falsely exposed `railroad` and `cynefin` style providers. It was restored to
    the lockfile Mermaid `11.15.0` commit `41646dfd43ac83f001b03c70605feb036afae46d`; under that
    source authority, the current theme coverage ledger remains consistent and no supported-family
    renderer defect was found.
  - A thirty-third HPD-080 Zed-feedback audit reconfirmed that current theme support covers common
    product-neutral host needs, while Zed-style exact palette cleanup remains host policy. The same
    external signal exposed a separate serious Flowchart issue: Zed PR `58325` fixed deep nested
    subgraph stack overflow in its fork. Local Flowchart cluster direction, descendant, anchor, and
    copy traversals now use explicit stacks, with a 512 KB stack-thread regression for 10,000 nested
    subgraphs.
  - A thirty-fourth HPD-080 resvg-safe smoke slice added a public `HeadlessRenderer` host-integration
    gate for the user Kanban metadata sample, the GitGraph merge sample, a dark-theme Flowchart
    sample, and representative supported-family fixtures. It checks XML parseability, absence of
    `foreignObject`, raster-unsafe CSS/token cleanup, non-empty style elements, and actual PNG
    conversion when the `raster` feature is enabled. No new renderer defect was found in that scan;
    treat it as a functional regression gate, not an all-fixture parity percentage.
  - A thirty-fifth HPD-080 all-supported resvg-safe audit slice resolved the Flowchart `layout.rs`
    conflict between the Zed PR `58325` backport shape and local explicit-stack traversal coverage,
    then ran the ignored supported-fixture audit. The same slice made empty Pie roots finite for
    headless/raster safety rather than preserving Mermaid's invalid `-Infinity` capture artifact.
    Its earlier Treemap bare-label-token assumption was later corrected by the forty-fourth slice:
    pinned Mermaid 11.15 rejects that syntax at parser/render time despite DB-layer style splitting
    being more tolerant.
  - A thirty-sixth HPD-080 extended-theme override slice fixed a host theme gap in official
    `neo/redux*` themes. Defaults still come from generated Mermaid 11.15 snapshots, but when users
    override source base keys such as `primaryColor`, `secondaryColor`, `background`, `lineColor`,
    or `mainBkg`, local theme expansion now recomputes source-backed visible derived keys consumed
    by current renderers. Direct derived-key overrides still win, matching Mermaid's
    `calculate(overrides)` order. The Flowchart regression intentionally confirms Redux node fill
    stays on `mainBkg` while the derived secondary color reaches visible edge-label CSS.
  - A thirty-seventh HPD-080 dark extended-theme slice deepened that same seam for
    `neo-dark` / `redux-dark*`: `primaryColor` now derives visible Requirement, Pie, and
    QuadrantChart palette keys, `redux-dark*` derives GitGraph `git0..7` / `gitInv0..7`, and
    explicit `gitN` colors derive matching inverses unless `gitInvN` is also explicit. This slice
    also fixed Pie layout to read `themeVariables.pie1..pie12` for slice/legend colors instead of
    using a hardcoded default palette.
  - A thirty-eighth HPD-080 audit tightened Journey coverage semantics: `arrowheadColor` is no
    longer counted as a public smoke visible signal because Mermaid 11.15 emits the
    `.arrowheadPath` CSS rule but current Journey marker DOM has no matching class. This was a
    measurement correction, not a renderer DOM change.
  - A thirty-ninth HPD-080 test-infra slice made the ignored all-supported `resvg-safe` audit
    filterable by `MERMAN_RESVG_SAFE_AUDIT_FAMILY` and `MERMAN_RESVG_SAFE_AUDIT_FILTER`. Render-only
    all-supported passed; unfiltered raster all-supported timed out, so future PNG-level triage
    should use filtered slices plus the representative raster smoke.
  - A fortieth HPD-080 test-infra slice tightened the raster branch of the resvg-safe fixture smoke
    to decode PNG output and reject contentful diagrams that rasterize as blank/all-background
    images. The gate is source-aware: header-only, accessibility-only, and title-only metadata
    fixtures still need valid SVG/PNG output but do not require non-background ink. A focused
    Architecture/Class/Sequence raster audit passed after that calibration.
  - A forty-first HPD-080 raster audit slice calibrated that source-content gate for remaining
    non-visual parser fixtures: Journey section-only, `packet-beta` header-only, Radar option-only,
    and Treemap no-value/classDef-only inputs no longer trigger false blank-output failures. The
    same pass found and fixed a real single-leaf Treemap defect: Mermaid 11.15's first Treemap color
    scale entry is `transparent` and default `cScaleLabel0` is white, so a single value leaf could
    rasterize as an all-background PNG. Local Treemap now keeps the transparent cell fill but uses
    `themeVariables.textColor` for leaf label/value inline fill only for the transparent-cell plus
    white-label combination when no explicit class/style fill overrides it.
  - A forty-second HPD-080 raster audit slice calibrated directive-only parser/metadata fixtures
    without changing renderer behavior. State `classDef`-only fixtures are style-registry tests,
    State bare `state foo` plus floating note alias samples are parser-only smoke cases, and
    Flowchart `click`-only input records interaction metadata without visible nodes. The raster
    source-content gate now skips `classDef`, `click`, and `linkStyle` metadata by themselves, plus
    those narrow State non-visual forms, while keeping visible State declarations and Flowchart
    `style ...` lines contentful. Full State and split-prefix Flowchart raster audits passed after
    the calibration.
  - A forty-third HPD-080 boundary renderability slice added a separate `resvg_safe` fixture smoke
    for `info`, `error`, and `zenuml`. These remain boundary/compatibility entrypoints rather than
    full supported-family style-provider parity claims. The `error` corpus now exercises lenient
    suppressed-error rendering, and all three boundary dirs reuse the XML, foreignObject,
    invalid-token, empty-style, and raster ink assertions from the public renderability gate.
  - A forty-fourth HPD-080 CI/compare diagnosis confirmed the reported Windows/macOS/Linux
    Sequence width and Class namespace snapshot failures were already fixed on current HEAD, then
    fixed the fresh structural compare regressions. Default-theme Pie now preserves Mermaid
    11.15's raw `pie1/#ECECFF` and `pie2/#ffffde` strings when unrelated `themeVariables` are
    supplied, matching `theme-default.js`. Treemap now rejects bare `classDef` style tokens such as
    `color` and renders the fixture through the suppressed error diagram, matching the pinned
    upstream SVG baseline instead of treating DB-layer `addClass` tolerance as parser parity.
  - A forty-fifth HPD-080 visible-signal audit tightened Journey and Timeline coverage after a
    hardcoded-color/source scan found inert CSS tokens being counted as visible public-smoke
    signals. Mermaid 11.15's Journey stylesheet emits inherited Flowchart-like rules for
    `.edgePath .path`, `.flowchart-link`, `.edgeLabel`, `.cluster text`, `.node ...`, and
    `.arrowheadPath`, but current Journey DOM does not emit those classes/elements. The public
    dark-theme smoke now counts only current visible Journey surfaces: generic line/label/legend
    text color, face fill, task/section fill types, and actor colors. Timeline smoke now counts
    visible section `cScale0` / `cScaleLabel0` / `cScaleInv0` colors instead of `.disabled` CSS from
    a source that emits no disabled DOM. Focused public render tests document both boundaries.
  - A forty-sixth HPD-080 visible-signal audit tightened Requirement coverage. Fresh Mermaid 11.15
    CLI evidence showed the ordinary Requirement render path emits current DOM for
    `.relationshipLine`, `.labelBkg`, `data-look="neo"`, and `outer-path`, while legacy provider
    rules such as `.reqBox`, `.reqTitle`, `.reqLabelBox`, and `.relationshipLabel` remain inert
    without matching DOM. Local Requirement now emits the `neo` DOM surfaces needed for
    `nodeBorder` to affect visible node/divider strokes, and the public dark-theme smoke no longer
    counts provider-only colors as visible signals.
  - A forty-seventh HPD-080 visible-signal audit calibrated Gantt public smoke coverage. The
    renderer already emitted Mermaid 11.15 ordinary-task, outside-label, and done-task selectors;
    the compact smoke source was too narrow and counted ordinary task colors while rendering only a
    done task. The smoke now includes a wide ordinary task, a narrow long-label task that emits
    `taskTextOutsideRight taskTextOutside0`, and a done task, so `taskBkgColor`,
    `taskBorderColor`, `taskTextOutsideColor`, `doneTaskBkgColor`, and `doneTaskBorderColor` are
    all backed by matching DOM.
  - A forty-eighth HPD-080 GitGraph official-theme slice confirmed the user-provided merge sample
    renders finite/readable default output, then fixed the source-backed theme gap it exposed:
    Mermaid 11.15 `git/styles.js` switches `neo` / `redux*` themes away from classic `git0`
    rules into `genColor(...)`. Local GitGraph now emits redux geometry rules, redux color-theme
    `borderColorArray` rules, neo first-branch/subsequent-branch rules, scoped neo gradient label
    backgrounds with matching `<defs>`, and `mainBkg` merge/reverse/highlight-inner fills without
    changing layout or root bounds.
  - A forty-ninth HPD-080 raster integration slice fixed the Ubuntu CI blank-PNG failure for the
    boundary `info` fixture. `info` is visible version-text output, so the fix stayed in the
    PNG/JPEG `usvg` path: raster options now bind missing generic font aliases to loaded faces,
    use a browser-like fallback resolver when a requested family such as `courier` is unavailable,
    and parse no-`viewBox` `max-width` SVGs with a matching default viewport width. Parity SVG
    output is unchanged.
  - A fiftieth HPD-080 Sequence autonumber slice fixed the user-reported activation-bound marker
    position defect. Pinned Mermaid 11.15 computes autonumber marker X from current
    `activationBounds(...)`, `fromBounds` / `toBounds`, arrow direction, and reverse-arrow type.
    Local Sequence SVG output now maintains the same render-pass activation-bounds stack, so the
    reported sample places numbers `2` and `4` inside the left edge of the Server activation rect
    and `5` inside the right edge, matching upstream behavior without changing Sequence
    root-width/font-metric residuals.
  - Continue HPD-080 by auditing remaining supported diagrams for missing style providers,
    unreadable text, blank/black output, and theme config that is parsed but not emitted. Do not
    chase visual parity beyond source-backed Mermaid rules or headless-style suitability.
