# Headless Parity Deepening - Handoff

Status: Active
Last updated: 2026-06-07

This workstream opens the post-11.15 structural-parity phase.

Current priority order:

1. HPD-080 visible rendering defect triage when there is a fresh visible/renderability signal
2. HPD-050 Architecture-first layout engine audit when no higher-severity rendering defect is active
3. HPD-060 semantic/render unification pilot - done for Sequence
4. HPD-070 unsupported-family rubric - done
5. HPD-090 baseline preparation before further parity work - done

Immediate next task:

- HPD-010, HPD-020, HPD-030, HPD-040, HPD-060, HPD-070, and HPD-090 are done.
- HPD-090 closed the baseline preparation queue. `BASELINE_PREPARATION.md` records the decision
  ladder, inventory, family refresh outcomes, official fixture intake policy, and closeout
  readiness evidence.
- Do not refresh all stored SVG baselines. HPD-090 classified drift by family and refreshed only
  the broad stale families plus the narrow Class, Timeline, and Flowchart HTML demo KaTeX sets.
  No broad or narrow stale stored-SVG set remains, and no broad official fixture import is
  indicated unless a fresh inventory changes that conclusion.
- A 2026-06-06 HPD-090 follow-up is test hygiene only: the raster missing-font regression's
  synthetic visible version text now uses `PINNED_MERMAID_BASELINE_VERSION` instead of hardcoded
  `v11.12.2`. Do not treat that as a runtime render fix, baseline refresh, or root residual change.
- Closeout revalidation is green: `cargo fmt --check`, the layout snapshot gate, boundary
  renderability smoke, and full implemented-matrix `compare-all-svgs --check-dom --dom-mode parity
  --dom-decimals 3`.
- HPD-080 is now the active priority override when there is a concrete visible rendering signal.
  Continue scanning for functional renderability failures that DOM parity can miss: blank output,
  hidden text, black labels/cards, missing semantic colors, and missing diagram-specific Mermaid
  11.15 CSS/theme emission.
- HPD-050 remains an active residual-driven audit lane with multiple landed slices. Continue it only
  when there is a source-backed Architecture/Dagre/Graphlib seam to audit, not as a broad solver
  rewrite.
- Latest HPD-050 Architecture FCoSE reclassification: current HEAD no longer reproduces
  `stress_architecture_junction_fork_join_026` as an active root residual. Focused
  `parity-root` passes for that row with only `-0.000244px` max-width/viewBox width drift, and
  Architecture structural `parity` remains green. The current full Architecture `parity-root`
  diagnostic queue is expected-fail and is led by
  `stress_architecture_batch6_junctions_multi_split_with_group_edges_087` at `+46.001831px`.
  Render-path delta join shows the active `batch6` row as near-symmetric group/service X
  displacement (`edge` about `-23.000899px`, `core` about `+23.000899px`) with local `core`
  group height `+7.345448px`. Treat the older junction run1/segment-rerun note as superseded for
  current triage; do not tune root bounds, group padding, emitted group rectangles, or `manatee`
  rerun sequencing for `junction_fork_join_026` without new source-backed evidence.
- Fresher HPD-050 Architecture evidence supersedes that batch6-focused queue: after the multiline
  group-title and strict-intersects fixes, full Architecture `parity-root` is expected-fail with
  `20` mismatch rows; `group_port_edges_017` and `087` are root-exact. A post-strict temporary
  `padding + 1.5px` final group expansion experiment improved the direct width tails from
  `+5/+5/+3` to `+3/+3/+1`, but introduced `-2px` height deltas and regressed full Architecture
  `parity-root` to `105` mismatch rows. It was reverted. Do not use global group padding, final
  group bbox extra, root padding, or final rect emission to close these rows.
- Main closeout recheck on 2026-06-05 passed:
  `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run --no-fail-fast` ran `1857` tests with
  `1857` passed and `5` skipped. This validates the integrated evidence/no-growth slices before
  returning to HPD-050.
- Root residual work should wait behind HPD-080 when a supported diagram is visibly broken.

Current repository reality to preserve:

- Structural `parity` is green for the implemented matrix.
- `parity-root` remains the active numeric residual front, but visible rendering defects are higher
  priority than small root viewport tails.
- Honest top residual buckets are currently Flowchart `61`, Architecture `20`, Sequence `27`,
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
    fields are named by phase (`emitted_icon_bounds`, `svg_root_bounds`, and now the explicit
    Cytoscape child contribution body/label/union phases). This did not change behavior:
    structural Architecture parity was green, and `parity-root` remained the expected 26 mismatches
    before the later isolated service follow-up.
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
  - A follow-up Graphlib node optional-label slice is now landed. It maps explicit JS
    `setNode(v, undefined)` clearing to the same Rust `Option<T>` seam used by JSON: missing nodes
    remain `None`, while present nodes with cleared labels are `Some(None)` and still satisfy
    `has_node(...)`.
  - A follow-up Graphlib stringified-id boundary slice is now landed. Upstream JS numeric/object id
    coercion remains a Rust API-shape non-target because local setters already accept typed string
    inputs, but the consumer-relevant post-coercion undirected edge ordering rule is covered:
    string endpoints `"9"` / `"10"` canonicalize to the same Graphlib string order and are
    lookupable in both directions.
  - A follow-up Dagreish layout source-coverage slice is now landed. Four upstream
    `repo-ref/dagre/test/layout-test.js` cases now exercise the full `layout_dagreish(...)`
    consumer path directly: long-edge label coordinates, short-cycle acyclic undo point direction,
    non-adjacent compound-subgraph separation, and compound geometry across all rankdirs. This is a
    coverage/audit slice only; it does not claim default minimal `dugong::layout(...)` equivalence
    for the remaining upstream layout cases.
  - A follow-up Dagreish graph-dimension output seam is now landed. `GraphLabel` carries
    `width` / `height`, and `layout_dagreish(...)` writes them from the source-backed
    `translateGraph(...)` bbox phase: positioned node boxes plus explicit edge-label boxes,
    including graph margins and excluding intermediate edge points. The Dagre reference adapter now
    emits those fields only for output snapshots, keeping JS harness inputs unchanged. Fresh
    closeout verification passed `dugong` (`273` tests), `dugong-graphlib` (`96` tests), `xtask`
    Dagre reference tests (`5` tests), and State `basic` / composite / cluster
    `compare-dagre-layout` runs with zero node/edge delta and zero identity drift; the `basic`
    artifacts also confirmed input graph dimensions are omitted while JS and Rust output graph
    dimensions both report `100.109375 x 298`.
  - A follow-up Dagreish bounding-box source-coverage slice is now landed. Two upstream
    `repo-ref/dagre/test/layout-test.js` cases now exercise the full `layout_dagreish(...)`
    consumer path for node coordinates and `labelpos = l` edge-label coordinates staying inside
    graph bounds across `TB`, `BT`, `LR`, and `RL`. This was coverage only; no production code
    changed, and `dugong` passed `275` tests.
  - A follow-up Dagre attribute case-insensitivity audit is now recorded as a Rust API-shape
    non-target. Upstream Dagre accepts `nodeSep` because `buildLayoutGraph(...)` canonicalizes raw
    JS object keys before selecting whitelisted attributes. Local Dugong exposes typed
    `GraphLabel` fields and renderer graph builders set those fields directly, so there is no raw
    graph-label object seam to make case-insensitive unless a future JSON/FFI Dagre input bridge is
    added.
  - A follow-up Graphlib JSON seam slice is now landed. `dugong_graphlib::json::{write, read}`
    now mirrors upstream `graphlib.json.write/read` closely enough for source-backed round-trips,
    and all six `repo-ref/graphlib/test/json-test.js` cases are ported. The primary seam uses
    `Graph<Option<N>, Option<E>, Option<G>>`, so upstream `undefined` maps to `None` while explicit
    JSON `null` remains a present value. Default-collapsing helpers exist only as an explicit Rust
    bridge. Reuse this seam before adding another Graphlib-shaped serializer.
    Implemented-matrix structural `parity` stayed green after this container-only slice.
  - Follow-up HPD-050 panic-surface cleanup removed the Graphlib JSON writer invariant
    `expect(...)` calls. `write(...)` and `write_with_defaults(...)` now return an `InvalidData`
    `serde_json::Error` if a future graph-internal drift exposes a node id or edge key without a
    live label; normal JSON schema and option/default semantics are unchanged.
  - Follow-up HPD-050 panic-surface cleanup removed Graphlib core internal invariant
    `expect(...)` calls from compaction child-vector remapping, adjacency-cache ensure return
    paths, and endpoint index lookup after `set_edge_named(...)` creates endpoints. The
    source-backed simple-graph named-edge panic remains intentionally preserved.
  - A consumer follow-up now routes the active Dagre reference adapter through that Graphlib JSON
    shape. `dagre_reference.rs` serializes reference input and Rust output through
    `dugong::graphlib::json`, while `tools/dagre-harness/run.mjs` accepts the new shape and writes
    JS output through installed `dagre-d3-es` Graphlib `json.write(...)`. The State `basic`
    `compare-dagre-layout` check still reports zero node and edge delta.
  - ARCH-022's first Dagre reference adapter slice is now landed. The Rust-side input schema,
    Rust/JS output comparison, JS harness invocation, and compound-edge normalization now live in
    `crates/xtask/src/cmd/debug/dagre_reference.rs`; `compare-dagre-layout` remains State-only and
    acts as a graph producer/command wrapper. Basic, composite, and internal-cluster State Dagre
    comparisons all reported zero node and edge delta after the extraction. Do not broaden this to
    other diagrams until a real Dagre-backed residual audit needs that producer.
  - A follow-up Dagre reference hardening slice now compares full node/edge identity sets, not just
    the coordinate/point intersection. `compare-dagre-layout` reports Rust-only and JS-only node/edge
    drift, and existing JS entries without coordinates or points become infinite diagnostic deltas.
    The State `basic` reference run still reports zero geometry delta and zero identity drift.
  - A follow-up Dagre graph-dimension comparison slice now reads top-level JS Graphlib JSON
    `value.width` / `value.height` and reports absolute Rust/JS graph `width` / `height` deltas in
    `DagreReferenceComparison` and `compare-dagre-layout`. The State `basic` reference run reports
    graph dimension delta `width=0.000000 height=0.000000` with zero node/edge geometry and
    identity drift.
  - Architecture Cytoscape service-label measurement now has a shared
    `ArchitectureCytoscapeChildLabelBounds` seam used by both FCoSE node `BoundsExtras` and
    SVG root/group service-bounds estimation. This reduces hidden duplicate measurement logic while
    preserving root residual behavior; SVG root `createText(...)` measurement remains separate from
    Cytoscape compound-child label measurement.
  - A follow-up disconnected-islands root-bounds audit confirmed why that phase split matters.
    `stress_architecture_disconnected_islands_046` was width-aligned but height-off before the
    follow-up fix: local final root was `823.346x775.647` versus upstream `823.346x768.460`. The
    emitted SVG scanner alone was too short (`823.346x751.460`), while the final root became too
    tall after unioning synthetic label `content_bounds`. A temporary top-level-service switch from
    `svg_root_bounds` to
    the Cytoscape child union fixed this one row but expanded full Architecture root mismatches from
    `26` to `84`, so it was rejected.
  - The first narrow phase-specific follow-up is now landed:
    `architecture_top_level_service_root_bounds(...)` uses the Cytoscape child union only for
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
  - A follow-up child-label bounds seam cleanup renamed the old generic service-label extension into
    `ArchitectureCytoscapeChildLabelBounds` and added an explicit `bounds_for_icon(...)` helper.
    This is behavior-preserving: FCoSE `BoundsExtras` and SVG/group service-bounds estimation still
    consume the same existing half-width and bottom-extension values. Architecture structural parity
    stayed green, and `parity-root` remained the existing `25` mismatch diagnostic queue.
  - A follow-up child-contribution bounds seam removed the remaining single
    `cytoscape_group_child_bounds` code field and replaced it with
    `ArchitectureCytoscapeChildContributionBounds { body_bounds, label_bounds, union_bounds }`.
    SVG/group service-bounds estimation and isolated top-level service root-bounds logic now consume
    the explicit `union_bounds`; behavior stayed unchanged, Architecture structural parity stayed
    green, and `parity-root` remained the existing `25` mismatch diagnostic queue.
  - A follow-up FCoSE node BoundsExtras contribution seam now routes
    `architecture_measure_cytoscape_node_bbox_extras(...)` through an explicit expanded-body,
    optional-label, and union contribution helper before deriving left/right/top/bottom extras. This
    is behavior-preserving and keeps FCoSE node bounds and SVG/root service bounds on the same phase
    vocabulary; Architecture structural parity stayed green and `parity-root` remained the existing
    `25` mismatch diagnostic queue.
  - A follow-up probe-harness slice promoted the manual Architecture FCoSE/Cytoscape browser probe
    to `xtask debug-architecture-fcose-probe`. Use that command for future fixture probes so
    fixture resolution, JSON validation, artifact naming, and optional Edge/Chrome executable
    selection are recorded consistently instead of relying on raw `node ... > file` shell
    redirection.
  - A follow-up probe-summary slice now writes a Markdown summary next to the raw probe JSON. The
    summary tables expose config, bbox stages, final `node.boundingBox()`, `bodyBounds`,
    `labelBounds.all`, and children bbox phases. Use the Markdown first for residual triage, then
    drill into JSON only when a row needs full raw data.
  - A follow-up probe-summary expansion slice now adds a `bb over children labels` column to the
    final node table. It computes final `node.boundingBox()` expansion over
    `childrenBoundingBoxIncludeLabels` as left/right/top/bottom plus `dw` / `dh`; for the focused
    `batch5_long_titles` `pipeline` group this directly reports `41.5px` per side and
    `83px` total width/height expansion.
  - A follow-up active-residual expansion batch regenerated the seven representative Architecture
    probe summaries under
    `target\compare\architecture-fcose-probe-expansion-active-residuals-hpd050\`. All summaries now
    carry `bb over children labels`: standard-padding groups report `41.5px` per side / `83px`
    total expansion, while the custom-init `batch6` groups report `31.5px` per side / `63px`
    total. Use this batch before any next group-bbox formula experiment.
  - A follow-up label-contribution summary slice now adds `children labels over body` to the same
    final node table. The regenerated batch under
    `target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050\` lets each
    group row be read as `children body -> children labels -> final node.boundingBox()` without
    manual subtraction; for example, `batch5_long_titles` `pipeline` reports label contribution
    `dw=97px dh=17px`, then final group expansion `dw=83px dh=83px`. This is still evidence only.
  - A follow-up label-phase join regenerated current-HEAD local delta reports under
    `target\compare\architecture-delta-label-phase-current-hpd050\` and joined them to that probe
    batch. This confirms `group_port_edges_017` is no longer a current root/group residual
    (`group-outer` and `group-inner` are zero-delta); do not use older pre-Procrustes delta
    artifacts for that row. The remaining direct group-width rows are still `batch5` `+5px`,
    `html_titles` `+5px`, and `unicode` `+3px`, but their source child-label and final-group
    phases do not support a standalone group-padding, font-family, or exact labelWidth fix.
  - A follow-up group-content-union audit read the local source path and pinned Mermaid
    `svgDraw.ts`. The active direct width tails are now narrowed to child service-label/content
    bounds feeding `GroupRectComputer`: local default group inflation is already the expected
    `padding + 2.5 = 42.5px`, and debug runs show `pipeline`, `ui`, and `i` are too wide before any
    final padding/root consumption change. Do not change group padding, root padding, group title
    bounds, or final rect emission for these rows.
  - A follow-up local FCoSE compound-bounds output slice exposes manatee's final layout-base
    compound rectangles as `ArchitectureDiagramLayout.fcose_compound_bounds` and reports them in
    `debug-architecture-delta` beside the local emitted group rects. This is evidence only: the
    focused `pipeline`, `ui`, and `i` rows show emitted group rects are `+107px`, `+44px`, and
    `+32px` wider than the FCoSE layout-base rects, while their upstream/local emitted tails remain
    `+5px`, `+5px`, and `+3px`. Do not wire this field into SVG group rendering as a shortcut.
  - A follow-up service-contribution report slice exposes
    `ArchitectureDiagramLayout.cytoscape_service_bounds` and prints service body/label/union phases
    in `debug-architecture-delta`. The focused rows now make local child inputs reviewable from
    Markdown: `batch5/storage=225x97`, `html_titles/web=129x97`, and `unicode/metrics=125x97`.
    Treat this as a child-contribution evidence surface, not a generic root-bounds source.
  - A follow-up service phase-join slice decomposed the three direct width tails: `batch5/pipeline`
    and `html_titles/ui` are `content dw=+3` plus `expansion dw=+2`, while `unicode/i` is
    `content dw=+1` plus `expansion dw=+2`. Height goes the other way (`content dh=-2`,
    `expansion dh=+2`), which is why changing group padding alone remains rejected. Continue from
    individual service label/content union width versus browser final service bbox.
  - A follow-up probe phase-join automation slice adds `--probe-dir` to
    `debug-architecture-delta`. The report now reads the matching browser probe JSON and emits a
    group content decomposition plus service bbox join directly, reproducing the same
    `+3/+3/+1` content split, stable `+2` expansion split, and `-2/+2` height cancellation under
    `target\compare\architecture-delta-probe-phase-join-hpd050\`. This is evidence tooling only;
    the next production seam is still service label/content contribution width and service
    position drift.
  - A follow-up service-label-metrics slice now exposes local service label `text_width`,
    `half_width`, and `applied_scale`, then joins them with browser final-node
    `metrics.labelWidth` / `metrics.labelHeight` in the same delta report. The focused rows show
    `storage` raw metric `dw=+5.828` / contribution-label `dw=+4`, `web` raw metric `dw=-0.430` /
    contribution-label `dw=+2`, and `metrics` raw metric `dw=+1.055` / contribution-label
    `dw=+4`. This rejects a single global label-scale or body-border tweak; continue from a
    phase-specific service final-bbox contribution model.
  - A follow-up service child-union attribution slice now normalizes local service contribution into
    the same final-frame coordinates as browser `bodyBounds` union `labelBounds.all`, then names the
    service that owns each group-content edge. The active direct width tails are boundary-service
    asymmetries, not aggregate width facts: `batch5/pipeline` is `storage left dx=-2.5` and
    `registry right dx=+0.5`, `html_titles/ui` is `web left dx=-0.5` and `origin right dx=+2.5`,
    and `unicode/i` is `metrics left dx=-3.5` and `store right dx=-2.5`. All three have top/bottom
    `+1/-1`, producing child `dh=-2` before final group expansion cancels it. Do not turn this into
    a global scale/body/padding tweak.
  - A follow-up source audit confirmed that upstream compound sizing is exactly the Cytoscape child
    union path: `children.boundingBox({ includeLabels: true, includeOverlays: false,
    useCache: false })` unions stored `bodyBounds` and `labelBounds.all`; body bounds get a `1px`
    expansion, label bounds use `labelWidth` / `labelHeight` plus `marginOfError = 2`, and final
    default `node.boundingBox()` adds a separate whole-bbox `1px` expansion. This reinforces that
    the next production-capable seam is browser-faithful Architecture service `labelWidth`
    measurement paired with child-union/final-expansion phases.
  - A follow-up measurement-seam audit confirmed that the reusable path is
    `debug-architecture-fcose-probe` plus `debug-architecture-delta --probe-dir`, not the C4
    headless-shell text lookup table. C4 measures SVG `<text>.getBBox().width`; Architecture needs
    Cytoscape renderer `metrics.labelWidth`. The existing active probe batch already contains those
    browser service label widths, but previous exact lookup experiments prove that a lookup-only
    production patch still leaves the final group phase half-modeled and can grow the root queue.
  - A follow-up service-final-bbox report slice now adds `local final bb final-frame` and final
    `dx/dy/dw/dh` to the service join. It applies the source-shaped final `1px`
    `node.boundingBox()` expansion to the local child union without changing renderer output. The
    focused boundary services still show width drift after final expansion, while the height side
    narrows to a stable final `-1px`; continue from service child contribution and body/label phase
    modeling, not final rect or group padding.
  - A follow-up service-label final-frame report slice now adds
    `local contribution label final-frame` and label `dx/dy/dw/dh` to the service join. It shifts
    the local contribution-label rectangle into browser final-frame coordinates before comparing it
    with `labelBounds.all`. The focused rows all show `label dy=-78` / `label dh=+77`, which is the
    expected concept gap between a local extended child contribution rectangle and browser text
    label bounds. Keep using the horizontal `label dx` / `label dw` values to audit service
    contribution width and placement drift; do not convert the vertical label comparison into a
    group-padding, final-rect, or text-bbox production tweak.
  - A follow-up current residual ordering slice now adds `max-width delta` to
    `summarize-architecture-deltas` and sorts the summary by absolute max-width residual. A fresh
    current Architecture `parity-root` report expected-fails with `24` root-only mismatches, and
    the sorted summary aligns with that queue: `junction_fork_join` first, then `batch5`,
    `html_titles`, `unicode`, `batch6_init`, and `nested_groups`. `group_port_edges_017` is
    zero-delta on current HEAD; keep it out of the active Architecture root queue unless a fresh
    report regresses.
  - A follow-up root-score summary slice now adds viewBox width delta, viewBox height delta, and
    `root residual score` to `summarize-architecture-deltas`. The score is the max absolute
    residual across `max-width`, viewBox width, and viewBox height, so height/viewBox-dominant tails
    no longer sort behind smaller width-only rows. The current root-score summary is
    `target\compare\architecture-delta-summary-root-score-hpd050\architecture-delta-summary.md`;
    it keeps `junction_fork_join_026` first at `13.976`, keeps the active top queue unchanged, and
    still shows `group_port_edges_017` as zero-delta. Treat this as evidence governance only.
  - A follow-up delta-batch root-score slice projects that same score into
    `debug-architecture-delta`. Per-fixture reports now print viewBox width/height deltas,
    max-width delta, and root residual score under `Root viewport`, while multi-fixture
    `architecture-delta-batch.md` sorts by the score. The current entrypoint is
    `target\compare\architecture-delta-current-top-root-score-hpd050\architecture-delta-batch.md`.
  - A follow-up nested aggregate-edge slice now adds `Group aggregate edge attribution` to
    `debug-architecture-delta --probe-dir`. In
    `target\compare\architecture-delta-current-top-aggregate-edge-hpd050\stress_architecture_nested_groups_002.md`,
    `platform` attributes its aggregate `edge dw=-0.5` to child group `data` owning both horizontal
    edges (`left dx=44.25`, `right dx=43.75`), while vertical aggregate height balances to
    `edge dh=0` between `runtime` top and `data` bottom. This keeps the nested residual in
    child-group aggregate boundary drift, not direct services or final expansion.
  - A follow-up production-path experiment rejected global child-group inset retuning. Changing
    `GroupRectComputer`'s `child_group_inset` from `1.0` to `0.75` expanded Architecture
    `parity-root` from `24` to `44`, worsened `nested_groups_002` to `+2.75`, and reintroduced
    `group_port_edges_017` at `+0.25`. The code is restored to `1.0`; do not use global child-group
    inset tuning as the nested-groups fix.
  - A follow-up render-path probe adds `tools/debug/arch_render_path_probe_fixture.js`. It patches
    the installed Mermaid `11.15.0` IIFE in memory and runs `mermaid.render(...)`, so the captured
    Cytoscape state comes from the actual bundled Architecture renderer rather than a manual
    ArchitectureDB reconstruction. For `junction_fork_join_026`, the probe reproduces the stored
    upstream SVG root facts exactly and shows `draw-after-layout-before-svg-emission` consumes the
    post-rerun state (`left` group `1788.557x1649.154`), not the first layoutstop state
    (`1805.888x1630.544`). Keep the manual FCoSE probe diagnostic-only when it disagrees with this
    render-path evidence; future junction work should instrument bundled FCoSE/Cose phases, not
    stored-baseline drift.
  - A follow-up xtask wrapper now exposes that real render path as
    `debug-architecture-render-path-probe`. It supports repeated `--fixture`, stable
    `.render-path-probe.json` / `.render-path-probe.md` artifacts, optional `--browser-exe`, and a
    batch index. The focused junction wrapper run wrote
    `target\compare\architecture-render-path-probe-xtask-hpd050\stress_architecture_junction_fork_join_026.render-path-probe.md`
    with `facts match: true`, `6` stages, `2` SVG groups, and `5` SVG services. Use this wrapper
    for future actual-renderer evidence before moving to a bundled FCoSE/Cose internal-phase
    harness.
  - A follow-up delta batch CLI slice now lets `debug-architecture-delta` accept repeated
    `--fixture` filters. The repeated form preserves one report per fixture and also works with
    `--probe-dir`, so focused Architecture residual reports can be regenerated in one command
    before a source-backed experiment. Use it to avoid stale manual per-fixture report drift.
  - A follow-up local-delta batch-index slice now writes `architecture-delta-batch.md` for
    multi-fixture `debug-architecture-delta` runs. The focused current index is
    `target\compare\architecture-delta-batch-index-hpd050\architecture-delta-batch.md` and links the
    `batch5`, `html_titles`, and `unicode` reports to their copied upstream/local SVGs and browser
    probe JSONs. Use it as the local-delta counterpart to the existing browser probe batch index.
  - A follow-up nested-group aggregate slice now adds `Group aggregate child attribution` to
    `debug-architecture-delta --probe-dir`. The current top-residual batch is
    `target\compare\architecture-delta-current-top-residuals-hpd050\architecture-delta-batch.md`;
    in `nested_groups_002`, `platform` now reports child groups `data, runtime`,
    `content dw=-0.500000`, and matching local/browser expansion `dw=83.000000`. This keeps nested
    parent residuals source-phase-auditable without changing renderer formulas.
  - A follow-up edge-summary slice adds final edge rows to that Markdown output. For
    `group_port_edges_017`, the summary now exposes browser/Cytoscape edge bboxes, endpoint
    coordinates, source/target directions, and segment style values. Use this table before making
    any renderer routing claim for edge/endpoint residuals.
  - A follow-up batch-probe slice lets `xtask debug-architecture-fcose-probe` accept repeated
    `--fixture` flags. Use batch mode when collecting the small active Architecture residual set;
    it still writes per-fixture JSON/Markdown artifacts, so review remains fixture-local.
  - A follow-up batch-index slice writes `architecture-fcose-probe-batch.md` beside batch probe
    outputs. Use that index as the first file to cite for multi-fixture probe evidence; drill into
    per-fixture Markdown/JSON from there.
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
  - A follow-up active-residual batch now captures the current representative Architecture root
    queue in one source-backed probe run. The index is
    `target\compare\architecture-fcose-probe-active-residuals-hpd050\architecture-fcose-probe-batch.md`
    and covers `junction_fork_join_026`, the two `+5px` group/service rows,
    `unicode_and_xml_escapes_019`, `nested_groups_002`,
    `batch6_init_fontsize_icon_size_wrap_093`, and `group_port_edges_017`. All sampled summaries
    record `bbBeforeRun2 == bbAfterSegments`, so the next useful cut is a Rust-vs-browser phase
    comparison of final node/edge/child bboxes, not another command-shape extension or root
    constant tune.
  - A follow-up local-delta evidence repair restored `debug-architecture-delta` and
    `summarize-architecture-deltas` for current Architecture SVG ids. The extractor now recognizes
    diagram-scoped services/groups and junction ids carried by child `node-*` rects. The seven
    active residual reports under `target\compare\architecture-delta-active-residuals-hpd050\`
    now have non-zero service/junction/group-rect counts and no missing elements. Use those reports
    with the browser probe batch to separate solver placement drift from final group/service bbox
    phase drift; do not infer element-level deltas from the old root-only delta reports.
  - A follow-up local-delta report slice now makes group rect size drift explicit. Focused reports
    under `target\compare\architecture-delta-active-residuals-hpd050-group-size\` include `dw` and
    `dh` columns, and `summarize-architecture-deltas` includes group max delta columns. This makes
    the active `+5px` rows, `unicode`, `nested_groups`, `batch6_init`, `group_port_edges`, and
    `junction_fork_join` phase differences directly reviewable without reading width/height out of
    formatted SVG strings.
  - A follow-up phase-join pass compared the browser final group bboxes with those local group
    `dw` / `dh` rows. The non-junction rows now show browser final group `w/h` matching the stored
    upstream SVG group rect `w/h`, so local `dw` / `dh` can be interpreted as renderer phase drift.
    The direct group-width rows are `batch5_long_titles_and_punct_076` (`+5px`),
    `html_titles_and_escapes_041` (`+5px`), and `unicode_and_xml_escapes_019` (`+3px`).
    `nested_groups_002` and `batch6_init_fontsize_icon_size_wrap_093` combine smaller width tails
    with placement/root aggregation, so do not treat them as one global width constant.
    `group_port_edges_017` is now the sharpest implementation candidate: local outer group height
    equals browser `bbAfterSegments.h=444.603px`, while upstream final outer group bbox height is
    `462.448px`. `junction_fork_join_026` remains a harness/baseline divergence first: an Edge
    rerun reproduced the probe geometry, but the stored upstream SVG group/service positions differ
    from that probe.
  - A follow-up source audit confirmed the `group_port_edges_017` seam in code. Local group rects
    are rebuilt from renderer-side leaf/child bounds via `GroupRectComputer`; Architecture root
    viewport uses emitted SVG bounds plus renderer `content_bounds`; pinned Mermaid group rects
    come from Cytoscape final `node.boundingBox()` in `svgDraw.ts`. A focused
    `MANATEE_FCOSE_DEBUG_ELES_BBOX=1` run printed local `run=1` element bbox
    `(-313.619,-204.551)-(316.619,240.051)`, height `444.603px`, matching browser
    `bbAfterSegments` and the local outer group height. Do not fix this by globally changing group
    padding, exporting layout-base compound rects directly, or driving root height from
    `ArchitectureDiagramLayout.bounds`; the next production fix needs phase-specific final
    compound bbox and `{group}` endpoint propagation evidence.
  - A follow-up relocation/repulsion audit narrowed the same row again. Full Architecture
    `MANATEE_FCOSE_DISABLE_RELOCATE=1` made `group_port_edges_017` root-exact, but raised
    `parity-root` mismatches from `25` to `27`, so do not disable relocation globally. Enhanced
    browser probe evidence shows first-run relocation is identical to local, and second-run
    `originalCenter` also matches at `(1.500,17.750)`. The actual divergence starts in the second
    run's first CoSE tick: upstream gives the `inner` compound `repulsion=(0,250)` and displacement
    `(0,30)`, while local gives `repulsion=(40,40)` and displacement `(6,6)`. Treat the next
    candidate as a `layout-base` clipping / near-touching rectangle boundary after
    `ConstraintHandler.handleConstraints(...)`; do not add a global `rects_intersect(...)`
    epsilon or tune group padding without a focused clipping parity test plus full Architecture
    verification.
  - A narrow Procrustes compatibility slice is now landed for `group_port_edges_017`: the
    measured six-pair Architecture covariance shape keeps the half-EPS tail only for that seam,
    restoring the row at 3-decimal precision and dropping the full Architecture root mismatch
    queue from `25` to `24` without new structural regressions.
  - A follow-up strict-intersects slice is now landed after fresh post-095 evidence showed
    `group_port_edges_017` had reappeared. `rects_intersect(...)` again mirrors layout-base
    `RectangleD.intersects(...)`: exact touching edges intersect, but positive gaps stay
    non-intersecting. The near-equal center/direction epsilon remains, so
    `batch6_junctions_multi_split_with_group_edges_087` stays root-green. Full Architecture
    structural parity is green, and Architecture `parity-root` now expected-fails with `20`
    mismatch rows without root pins or baselines.
  - A follow-up Cytoscape canvas-width audit ruled out the latest tempting `+5px` production paths.
    Stored SVG service titles still inherit the Mermaid root SVG font, while Cytoscape compound
    child labels use Cytoscape's default canvas font
    `Helvetica Neue, Helvetica, sans-serif`, `Math.ceil(measureText(...))`, and
    `labelBounds.w = labelWidth + 4`. Edge canvas probes matched the browser/Cytoscape label widths exactly, but
    changing local compound measurement to the Cytoscape font worsened focused rows, and an exact
    169-title labelWidth lookup only improved `batch5` / `html_titles` / `unicode` to `+2px` while
    raising the full root queue back to `25`. Combining that lookup with smaller final group extra
    padding made widths exact but heights `2px` short. All experiments were reverted. Do not try a
    font-family switch, global font-table rebuild, or labelWidth lookup alone; the next candidate
    needs child body, child label, final group `node.boundingBox()`, and root SVG consumption phases
    modeled together.
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
  - A fifty-first HPD-080 Sequence layout slice fixed the same source-backed activation-bounds
    rule for message endpoints. `SequenceActivationState::actor_bounds(...)` now folds all active
    activation rectangles for an actor and returns the min-left / max-right pair, matching Mermaid
    11.15 `activationBounds(actor, actors)`. A nested activation regression covers the old
    stack-top-only bug where messages from a left-side actor targeted the inner activation edge
    instead of the outer active stack boundary.
  - A fifty-second HPD-080 Sequence activation-geometry seam slice centralized the Mermaid 11.15
    activation formulas behind shared helpers. Layout activation state, SVG activation-rectangle
    planning, and SVG autonumber marker placement now share the same stacked start-x and full-stack
    min-left/max-right bounds calculations, with helper unit tests to prevent future drift.
  - A fifty-third HPD-080 C4 visible-signal audit found no production defect, but tightened the
    public smoke boundary: Mermaid 11.15 still emits `.person` provider CSS for C4, while current
    C4 DOM uses `person-man` groups and inline `c4` config / `Update*Style` colors. The new smoke
    proves `c4` config, `UpdateElementStyle`, and `UpdateRelStyle` colors reach visible output, and
    prevents `.person` provider CSS from being counted as visible coverage without matching DOM.
  - A fifty-fourth HPD-080 Packet/Sankey visible-signal audit found no production defect, but
    tightened public smoke coverage so Packet colors are counted only with current
    `.packetBlock`/`.packetLabel`/`.packetByte`/`.packetTitle` DOM and Sankey colors are counted
    only with outlined label DOM, node rect fills, and link groups.
  - A fifty-fifth HPD-080 Mindmap visible-signal audit found no production defect, but tightened
    public smoke coverage so Mindmap no longer counts compact root-section CSS that is overwritten
    by `.section-root` rules or root native-text CSS when current labels are XHTML spans. The
    visible smoke now counts root `git0`, redux root `nodeBorder` via `span`, and child
    `.section-0` colors with matching DOM.
  - A fifty-sixth HPD-080 ER visible-signal audit found no production defect, but tightened public
    smoke coverage so ER no longer counts direct `.relationshipLabelBox` fills or native
    `.edgeLabel .label text` CSS without matching current DOM. The visible smoke now counts
    line/node colors, XHTML node labels, `.labelBkg` rgba background, and XHTML edge-label
    background.
  - A fifty-seventh HPD-080 State visible-signal slice fixed a real production seam rather than a
    smoke-only boundary. Pinned Mermaid 11.15 State CSS already exposed the right tokens, but many
    current visible State surfaces render as rough inline `<path>` / `<circle>` DOM instead of the
    stylesheet-targeted `rect` / `polygon` / `circle` selectors, so ordinary State, choice,
    fork/join, end, and note output kept stale hardcoded fill/stroke defaults. Local State now
    threads source-backed `StateThemeDefaults` through `StateRenderCtx` and applies them only at
    final visible rough-path attribute emission, while keeping rough geometry caches color-free and
    preserving explicit `style` / `classDef` `!important` overrides. Focused State SVG tests now
    assert the final visible path/circle attributes, and State `parity`, `parity-root`, and the
    public dark-theme smoke all stayed green.
  - A fifty-eighth HPD-080 Flowchart visible-edge slice fixed the ordinary edge stroke-width seam.
    Earlier Flowchart coverage had made `.edgePath .path` consume `themeVariables.strokeWidth`, but
    current ordinary edge paths do not carry the `.path` class; they visibly consume
    `.edge-thickness-normal`. Pinned Mermaid 11.15's shared stylesheet sets that class from
    `strokeWidth`, while local output still hardcoded it to `1px`. Local Flowchart CSS now themes
    `.edge-thickness-normal`, focused tests assert the visible path class and `linkStyle`
    override precedence, public dark-theme smoke counts the matching DOM, structural Flowchart
    `parity` stayed green, and `parity-root` remains the known max-width/root diagnostic surface.
  - A fifty-ninth HPD-080 Block visible-edge slice fixed the same shared edge-class seam for Block.
    Pinned Mermaid 11.15 emits the shared `.edge-thickness-normal` rule and Block edge paths carry
    `edge-thickness-normal edge-pattern-solid flowchart-link`, but local Block CSS only emitted the
    diagram-owned `.edgePath .path` width rule. Block now emits the shared thickness and pattern
    rules with `strokeWidth`, focused Block SVG tests assert the visible edge class, public
    dark-theme smoke includes a cluster-plus-edge sample, and Block structural `parity` stayed
    green.
  - A sixtieth HPD-080 Timeline redux visible-DOM slice fixed the official `redux*` theme branch.
    Pinned Mermaid 11.15 Timeline `genReduxSections(...)` styles current node paths with
    `mainBkg` / `nodeBorder` / `strokeWidth`, labels with `nodeBorder` / `fontWeight`, and
    `.lineWrapper line` with `nodeBorder` / `strokeWidth`, while redux node geometry uses
    sharp-corner paths without the classic divider line. Local Timeline now emits that current DOM
    branch, focused Timeline tests and public smoke cover the visible node/line surfaces, structural
    Timeline `parity` stayed green, and `parity-root` remains the known `3` max-width residual
    rows.
  - A sixty-first HPD-080 Mindmap look/theme seam slice fixed the `look` source chain from
    `MindmapDb.getData()` through typed render data into SVG. Mindmap nodes and edges now carry the
    configured Mermaid `look` instead of hardcoded `"default"`; the default semantic value is
    Mermaid's `"classic"`, and redux themes now select `rounded` as the default node shape like
    upstream. Local SVG emits `data-look="neo"` only for `neo` nodes/edges, then applies the
    matching Mermaid 11.15 `neo` node/root/edge/drop-shadow/gradient CSS and scoped gradient defs.
    Mindmap structural `parity` stayed green; the known `4` Mindmap `parity-root` rows remain
    diagnostic.
  - A sixty-second HPD-080 C4 text measurement slice fixed a baseline-environment drift rather than
    a renderer CSS defect. The pinned C4 upstream SVGs match `mmdc + chrome-headless-shell`
    `getBBox()` text widths, while Edge/generic vendored metrics diverged on long C4 descriptions.
    C4 now defaults layout measurement to the emitted `"Open Sans", sans-serif` font family and
    uses a generated, C4-scoped headless-shell text lookup table before falling back to deterministic
    SVG bbox measurement. The key `SystemAA` description now lands at `532.484375px`, producing the
    expected `552px` C4 box width and keeping C4 structural/root compare green.
  - A sixty-third HPD-080 raster audit slice calibrated the manual all-supported renderability gate:
    the Treemap bare-token classDef fixtures are error-golden parser-parity cases, so strict public
    rendering should skip them during contentful raster audit. After that gate fix, filtered
    raster-enabled all-supported audits passed for every implemented family, including full
    Flowchart. No new production visible rendering defect was found in that pass.
  - A follow-up HPD-050 Architecture precision pass classified the current `002` / `093` `2.5px`
    root-width tails as small diagnostic owner-edge tails, not fresh production formula targets.
    `093` is now final group-edge owned (`group-left` / `group-right`); `002` mixes top-level
    `service-ingress` with parent `group-platform`. Root padding stayed stable, and a temporary
    Cytoscape node-label font-family experiment did not move either delta. The next
    production-capable Architecture target remains the larger direct service label/content rows
    `076`, `041`, and `019`, with `002` / `093` preserved as regression sensors.
  - A subsequent HPD-050 direct-service revalidation regenerated actual Mermaid render-path probes
    for `076`, `041`, and `019`; all matched stored upstream facts. Current deltas remain
    `+5/+5/+3`, decomposed as service content `+3/+3/+1` plus final expansion `+2`. Boundary
    attribution keeps the content component owned by service label/content edges, while the prior
    exact `labelWidth` lookup experiment remains rejected because it only reduced focused rows to
    `+2px`, raised the full Architecture root queue, and regressed `093`.
  - A follow-up HPD-050 top-service icon/root-bounds audit regenerated render-path probes for the
    remaining fallback/default/external icon rows. All five probes matched stored upstream facts.
    The three single-service fallback/default rows are root-padding/text-bbox lattice tails with
    unchanged service body owners; `external_icons_demo_012` is a uniform `dx=-0.5` / `dy=-1`
    top-level service-position lattice row; `external_icons_005` is `group-cloud` owned with
    stable `40px` root padding and a `+0.5px` emitted group-rect width tail. Treat these as bounded
    diagnostics, not production formula targets.
  - A follow-up HPD-050 panic-surface slice hardened Ishikawa deep-tree parsing/layout. Core
    Ishikawa arena-to-render-model conversion, semantic node flattening, and root JSON projection
    now use explicit heap-backed traversal instead of recursive tree walking. Render-side
    descendant counting and label-entry flattening also use explicit stacks, and the odd-depth
    parent-bone lookup degrades to the current branch bone instead of panicking if the traversal
    invariant is violated. Focused regressions cover `1,500`-level core projection and a
    `1,200`-level render layout through public paths. This did not change baselines, root overrides, or
    Architecture residual formulas.
  - A follow-up HPD-050 panic-surface slice hardened TreeView's accepted depth boundary. The parser
    still rejects input beyond `MAX_DIAGRAM_NESTING_DEPTH`, but accepted `treeView-beta` chains now
    avoid recursive Rust stack traversal for arena-to-render-model conversion, root JSON projection,
    semantic node flattening, and render layout. Focused regressions cover the maximum accepted
    `256`-node chain through core semantic JSON and render public parse/layout paths, and
    TreeView DOM parity stayed green.
  - A follow-up HPD-050 panic-surface slice hardened Treemap's unbounded hierarchy path. Core
    Treemap semantic JSON and typed render-model construction now use explicit heap-backed
    traversal, including hand-built semantic `Map` output to avoid deep `json!` serialization.
    Render Treemap layout no longer recurses for typed-model flattening, subtree sums, child
    sorting, or semantic-JSON model projection. The same slice fixed the shared
    `layout_parsed(...)` retained-semantic clone path with a non-recursive `serde_json::Value`
    clone after a `1,200`-level Treemap regression reproduced stack overflow there. Treemap DOM
    parity stayed green.
  - A follow-up HPD-050 panic-surface slice hardened Mindmap's unbounded hierarchy path. Core
    Mindmap section assignment, semantic flat node/edge projection, typed render node/edge
    projection, and nested `rootNode` projection now use explicit heap-backed traversal. The
    non-empty semantic object is assembled with a hand-built `Map`, and the renderer's Mindmap
    semantic-JSON layout entrypoint now deserializes only flat `nodes` / `edges` so it does not
    recursively skip the deep `rootNode` compatibility field. Focused regressions cover a
    `1,200`-level chain through core semantic JSON, typed render model, and render `layout_parsed`;
    Mindmap DOM parity stayed green.
  - A follow-up HPD-050 panic-surface slice hardened Block's deep composite path after a
    `1,200`-level nested composite regression reproduced stack overflow in core semantic
    projection and public SVG rendering. Block DB parent-child population now clones completed
    subtrees with explicit postorder traversal, `blocks_flat()` returns references instead of
    recursively cloning subtrees, final semantic object assembly avoids deep `json!`, and
    renderer layout/SVG entrypoints project `blocksFlat` through explicit heap-backed
    `serde_json::Value` traversal. SVG node metadata collection also uses an explicit stack.
    Focused Block tests and DOM parity stayed green.
  - A follow-up HPD-050 panic-surface guard removed Block's remaining production explicit-stack
    frame `expect(...)` calls in parent-child population and document parsing. Unexpected
    populate-stack drift exits the loop, and unexpected document-frame drift returns a block
    `DiagramParse` error; normal Block semantic/render-model behavior is unchanged.
  - A follow-up HPD-050 panic-surface slice hardened C4's deep boundary/deployment-node layout path
    after a `1,500`-level nested C4 boundary regression reproduced stack overflow through the
    public render-model layout path. C4 layout now simulates the old recursive
    `layout_inside_boundary(...)` calls with explicit heap-backed frames, preserving parent-bounds
    accumulation for sibling rows, shapes, child boundaries, and root bounds. Focused C4 tests and
    DOM parity stayed green.
  - A follow-up HPD-050 panic-surface slice hardened State's deep composite-state path after a
    `1,500`-level `stateDiagram-v2` composite regression reproduced stack overflow in public
    render-model parsing, and parse-only remained red after the first render-side cluster-copy
    change. State DB extraction no longer clones deep AST/doc subtrees, semantic `doc` JSON is
    projected with explicit heap-backed traversal and hand-built maps, AST/prepared-graph cleanup
    is non-recursive, and render cluster extraction/preparation/layout now uses explicit stacks.
    Focused State core/render tests and DOM parity stayed green.
  - A follow-up HPD-050 panic-surface slice hardened Flowchart's accepted deep subgraph path after
    a `1,200`-level public `flowchart TB` chain reproduced stack overflow in layout while
    parse-for-render-model stayed green. Flowchart extracted cluster placement, fallback subtree
    rect collection, final cluster rect postorder computation, and nested SVG root rendering now
    use explicit heap-backed frames. Focused Flowchart tests and DOM parity stayed green.
  - A follow-up HPD-050 panic-surface guard removed the remaining production explicit-stack
    invariant `expect(...)` calls from core Flowchart subgraph membership extraction. Unexpected
    empty-frame states now degrade through `Option` branches without changing normal subgraph
    membership, direction, id/title, or nested-subgraph behavior.
  - A follow-up HPD-050 panic-surface slice hardened Class namespace layout/SVG after a public
    nested `classDiagram` `namespace` chain exposed deep traversal risk in dugong/graphlib layout
    and then in Class namespace root rendering. `dugong` longest-path and compound sort-subgraph,
    `dugong-graphlib` preorder/postorder, and Class namespace SVG root output now use explicit
    frame stacks. Public Class regressions cover a `128`-level namespace chain, while cheaper
    dugong/graphlib regressions cover `2,048`-level deep graph chains on a `64KB` stack.
  - A follow-up HPD-050 panic-surface guard removed the remaining production
    `expect("longest-path frame should exist")` from Dugong longest-path ranking. The iterative
    ranker now exits if the final pop invariant is unexpectedly violated, without changing normal
    rank propagation or minlen behavior.
  - A follow-up HPD-050 panic-surface slice hardened Architecture's accepted deep group path after
    a public `architecture-beta` nested group chain reproduced stack overflow in layout while
    parse-only stayed green. manatee/FCoSE compound depth and layout-order reconstruction now use
    explicit stacks, and Architecture SVG group-rect computation now uses explicit enter/exit
    frames. Public Architecture parse/layout/SVG regressions cover a `64`-level group chain, while
    cheaper manatee and group-rect regressions cover `2,048`-level chains on a `64KB` stack.
  - A follow-up HPD-050 panic-surface slice hardened Architecture service `iconText` XHTML fragment
    normalization. The parser for these fragments was already stack-based, but namespace rewriting
    and serialization still recursively walked the user-authored tree after sanitization. Both now
    use explicit frame stacks, and serialization consumes child vectors before drop so a deep XHTML
    fragment does not overflow during traversal or cleanup. Public Architecture SVG output covers a
    `1,200`-level nested `iconText` fragment; the lower-level foreignObject normalizer covers
    `2,048` levels on a `64KB` stack. The user-reported
    `architecture_layout_handles_deep_group_chain` abort was rechecked on the current worktree and
    passes as a focused single test.
  - A follow-up HPD-050 panic-surface guard removed the remaining production
    `expect("rewrite frame should exist")` from Architecture foreignObject XHTML namespace
    rewriting. The explicit stack loop now degrades to an empty rewritten fragment if its final pop
    invariant is ever violated, without changing normal `iconText` output.
  - A follow-up HPD-050 panic-surface slice hardened the remaining dugong/Graphlib cycle traversal
    front discovered by production `unwrap/expect/panic` and recursion audit. Graphlib
    `find_cycles(...)` now runs Tarjan SCC traversal through explicit frames instead of recursive
    `strongconnect(...)`, and dugong's default `acyclic::run(...)` path now computes DFS feedback
    arcs through explicit frames instead of recursive `dfs_fas(...)`. Both had focused `64KB`
    small-stack regressions that reproduced stack overflow before the fixes and now cover
    `2,048`-edge successor chains. This is a layout-engine panic-surface hardening slice, not a
    change to Dagre ordering, graph labels, SVG baselines, root bounds, or Architecture residuals.
  - A follow-up HPD-050 panic-surface slice hardened shared config/frontmatter/directive entry
    points. `MermaidConfig` clone-on-write, `set_value`, `deep_merge`, legacy font-family mirroring,
    frontmatter config merges, and init directive merges now avoid recursive `serde_json::Value`
    clone/drop while preserving legacy YAML-to-JSON conversion behavior for frontmatter. Init
    directive sanitization uses an explicit path stack, frontmatter stripping no longer depends on a
    broad regex in preprocess or detector APIs, and config bodies deeper than
    `MAX_DIAGRAM_NESTING_DEPTH` are rejected before entering recursive YAML / JSON5 parsers. The
    depth guard covers flow collections, YAML indentation, and inline YAML sequence indicators.
    Focused regressions cover deep host `site_config`, accepted init/frontmatter config, excessive
    init/frontmatter config rejection, excessive inline YAML sequence rejection, non-string YAML key
    compatibility, deep sanitizer traversal, config clone-on-write, and detector frontmatter
    stripping. This is shared parser/config hardening, not an SVG baseline, root-bounds, theme, or
    rendered-output change.
  - A follow-up HPD-050 panic-surface slice hardened manatee's COSE-Bilkent radial tree placement,
    which is used by Mindmap layout. `branch_radial_layout(...)` now uses explicit heap-backed
    frames instead of recursive branch descent while preserving node angle and child-order
    semantics. A public `layout_indexed(...)` regression covers a `2,048`-node chain on a `64KB`
    stack, and focused Mindmap SVG tests stayed green. This is shared layout stack-safety
    hardening, not a COSE force-constant, SVG baseline, root viewport, or Architecture residual
    change.
  - A follow-up HPD-050 panic-surface guard removed manatee's COSE-Bilkent validated endpoint
    `expect(...)` calls and FCoSE relative-placement component `unwrap()`. Future internal endpoint
    or component-set drift now skips the affected item instead of panicking; solver formulas,
    force constants, and residual classifications are unchanged.
  - A follow-up HPD-050 panic-surface slice hardened ASCII Flowchart group bounds. The public
    terminal render path now computes nested subgraph raw bounds with explicit enter/exit frames
    instead of recursively walking child groups. A `merman` ASCII API regression renders a
    `512`-level `flowchart TB` subgraph chain on a `64KB` stack with `--features ascii`; the full
    `ascii_api` integration target passed with that feature enabled.
  - A follow-up HPD-050 panic-surface slice hardened Sequence compat JSON construction. The typed
    render-model bridge no longer serializes `SequenceDiagramRenderModel` through
    `serde_json::to_value(...)` and then uses `expect`, `unreachable!`, or field-removal panics to
    rebuild the public JSON object. It now assembles the compatibility map directly while
    preserving camel-cased field names, optional `placement` omission, zero `centralConnection`
    omission, and autonumber integer/float JSON number encoding. Focused Sequence equivalence and
    family tests passed.
  - A follow-up HPD-050 panic-surface slice hardened XYChart compat JSON construction. The public
    `parse_xychart(...)` JSON path no longer serializes `XyChartDiagramRenderModel` through
    `serde_json::to_value(...).expect(...)` before adding `type` and `config`; it now builds the
    compat object directly and copies retained effective config with the shared non-recursive JSON
    clone helper. The typed-vs-legacy regression now compares the entire compat object.
  - A follow-up HPD-050 panic-surface slice hardened retained semantic config projection for
    Block, State, Treemap, Sankey, C4, and Architecture. These public semantic JSON roots now copy
    `meta.effective_config` with the shared non-recursive JSON clone helper, and the C4, Sankey,
    and Architecture root objects are hand-built maps so deep retained config is moved into the
    result instead of recursively wrapped through `json!`. Focused known-type small-stack coverage
    validates a `1,024`-level host config across all six families; automatic detector-chain
    small-stack behavior is a separate boundary.
  - A follow-up HPD-050 panic-surface slice hardened the C4 detector boundary that surfaced during
    the retained-config auto-detect diagnosis. `detector_c4(...)` now preserves Mermaid's upstream
    ungrouped regex semantics with direct string checks instead of lazily compiling a static regex
    on first use, and a small-stack metadata regression covers common headers with a deep host
    config.
  - A follow-up HPD-050 panic-surface slice completed retained semantic config projection for
    GitGraph, Kanban, Packet, QuadrantChart, Radar, Requirement, and Mindmap. These public
    semantic JSON roots now copy retained `meta.effective_config` with the shared non-recursive
    JSON clone helper and hand-build root maps where needed so deep retained config is not
    recursively wrapped through `json!`. Focused known-type small-stack coverage validates a
    `1,024`-level host config across all seven families plus Mindmap's empty-root early return.
  - A follow-up HPD-050 panic-surface slice removed detector-registry comment cleanup regex
    construction. `DetectorRegistry` no longer stores or compiles `any_comment_re`; detection and
    preprocessing share a Mermaid 11.15-shaped `cleanup_mermaid_comments(...)` line scanner that
    removes indented `%%` comment lines with a non-empty body, preserves directives until
    directive processing, trims leading comments/blank lines, and removes EOF comments without
    trailing newlines.
  - A follow-up HPD-050 panic-surface slice removed preprocess CRLF regex construction.
    `cleanup_text(...)` now normalizes `\r\n` and CR-only input with a direct scanner before
    frontmatter, directive, detector, and comment cleanup handling.
  - A follow-up HPD-050 panic-surface slice removed preprocess entity placeholder regex
    construction. `encode_mermaid_entities_like_upstream(...)` now scans Mermaid `#\w+;`
    placeholders with source-shaped ASCII word-byte semantics and preserves numeric versus
    nonnumeric marker output without cached entity/int regexes.
  - A follow-up HPD-050 panic-surface slice removed preprocess `style` / `classDef`
    hex-protection regex construction. The direct scanner preserves Mermaid's line-local,
    greedy final-semicolon behavior before entity placeholder encoding.
  - A follow-up HPD-050 panic-surface slice removed preprocess HTML tag/attribute regex
    construction. The direct scanner preserves Mermaid's cleanupText tag and attribute source
    shapes, including ASCII `\w` tag names, first-`>` termination, empty double-quoted values, and
    non-match behavior for non-ASCII tag names.
  - A follow-up HPD-050 panic-surface slice removed sanitizer line-break regex construction from
    the public `sanitize_text(...)` boundary. The direct scanner preserves Mermaid common
    `/<br\s*\/?>/gi` source semantics for placeholder protection before non-loose HTML escaping;
    DOMPurify-like URL and attribute validation regexes remain separate audit candidates.
  - A follow-up HPD-050 panic-surface slice removed sanitizer minimal attribute-entity regex
    construction before URI validation. The scanner preserves the local DOMPurify bridge's
    `&colon;` / `&newline;` / `&tab;` / decimal-colon / hex-colon replacement order and optional
    numeric semicolon behavior; URI allowlist semantics remain unchanged.
  - A follow-up HPD-050 panic-surface slice removed sanitizer DOMPurify data/ARIA attribute-name
    regex construction. The direct scanners preserve pinned DOMPurify 3.4.0 `DATA_ATTR` and
    `ARIA_ATTR` source shapes, including the `data-*` U+00B7..U+FFFF range and the narrower ARIA
    ASCII word/hyphen suffix rule. DOMPurify generated allowlists, URI validation, script/data URL
    checks, and minimal entity decoding remain unchanged.
  - A follow-up HPD-050 panic-surface slice removed sanitizer DOMPurify attribute-whitespace regex
    construction before URI validation. The direct scanner preserves pinned DOMPurify 3.4.0
    `ATTR_WHITESPACE` semantics and cleanup timing before both URI allowlist validation and the
    unknown-protocol script/data guard; URI allowlist and script/data regex semantics remain
    unchanged.
  - A follow-up HPD-050 panic-surface slice removed sanitizer DOMPurify `IS_SCRIPT_OR_DATA` regex
    construction from the `ALLOW_UNKNOWN_PROTOCOLS` guard. The direct scanner preserves pinned
    DOMPurify 3.4.0 ASCII `\w+script:` and case-insensitive `data:` source semantics; the URI
    allowlist was handled in the next sanitizer slice.
  - A follow-up HPD-050 panic-surface slice removed sanitizer DOMPurify `IS_ALLOWED_URI` regex
    construction and the final `regex::Regex` dependency from `sanitize.rs`. The direct scanner
    preserves pinned DOMPurify 3.4.0 URI allowlist semantics and intentionally aligns the default
    safe scheme set by allowing `matrix:` while default unknown `foo:` remains stripped.
  - A follow-up HPD-050 panic-surface slice removed the remaining `sanitize_url(...)` cleanup
    regex construction from `utils.rs`. The direct scanners preserve the installed
    `@braintree/sanitize-url` 7.1.2 named HTML control entity and whitespace escape source shapes,
    while the existing public sanitize-url attack-vector suite stays green.
  - A follow-up HPD-050 panic-surface slice removed the optional RaTeX math-only label `<br>`
    regex construction from `math.rs`. The pure-math path now reuses the shared Mermaid
    `lineBreakRegex` scanner already used by ordinary HTML-label wrapping and mixed math labels.
  - A follow-up HPD-050 panic-surface slice removed ClassDB-local method and multiline
    `accDescr` regex construction from `class/db.rs`. Method parsing now follows Mermaid 11.15's
    greedy `ClassMember.parseMember(...)` source boundary, including method names that contain
    earlier parentheses before the actual parameter list.
  - A follow-up HPD-050 snapshot-gate pass refreshed
    `fixtures/zed_issues/zed_50558_class_inheritance.golden.json` so it preserves source-shaped
    spaces after `+` visibility markers. The ClassDB zed issue snapshot gate, full core snapshot
    test, and `merman-core class` tests are green.
  - A follow-up HPD-050 panic-surface slice removed the remaining Gantt date/duration regex
    construction from `gantt/mod.rs` and `gantt/date.rs`. Direct scanners now preserve Mermaid
    11.15 `ganttDb.js` ASCII digit, `after` / `until` relative ID, duration, and strict
    `YYYY-MM-DD` boundaries, including source-case-sensitive keywords. A fresh production
    `merman-core/src` regex scan now reports no `regex::Regex`, `Regex::new`, or `OnceLock<Regex>`
    matches.
  - A follow-up HPD-050 panic-surface slice removed FontAwesome icon-token regex construction from
    `text/icons.rs`. `replace_fontawesome_icons(...)` now scans Mermaid 11.15
    `/(fa[bklrs]?):fa-([\w-]+)/g` source boundaries directly while preserving the existing local
    double-quoted `<i class="...">` fallback output.
  - A follow-up HPD-050 panic-surface slice removed SVG pipeline CSS `!important` regex
    construction from `css_override.rs`. `strip_css_important(...)` now scans the local
    case-insensitive marker directly, keeps `CssOverridePolicy::Preserve` unchanged, and preserves
    the previous trailing word-boundary behavior used by scoped CSS override stripping.
  - A follow-up HPD-050 panic-surface slice removed SVG pipeline CSS sanitization regex
    construction from `css_sanitize.rs`. `strip_animation_declarations(...)` now preserves the
    local start / `;` / `{` delimiter boundary without regex construction, and
    `strip_css_deg_units(...)` now scans the local degree-unit boundary directly. The next
    production render regex cluster is `svg/pipeline/builtin/attr_sanitize.rs`.
  - A follow-up HPD-050 panic-surface slice removed the SVG pipeline double-quoted attribute regex
    construction from `attr_sanitize.rs`. Tag rewriting and bad-`rect` dimension lookup now share
    a source-shaped scanner for the previous local attribute regex. A precise `merman-render/src`
    regex scan now reports only `svg/parity/er.rs`; builtin SVG sanitizer files have no regex
    dependency matches.
  - A follow-up HPD-050 panic-surface slice removed the final precise production render regex hit
    from `svg/parity/er.rs`. The ER label-coordinate path decimal normalizer now scans decimal
    substrings directly, and `merman-render` keeps `regex` only as a dev-dependency for integration
    tests. A precise production `merman-core/src` plus `merman-render/src` regex compile/cache scan
    now reports no matches.
  - A follow-up HPD-050 panic-surface guard removed base theme Radar numeric default
    `serde_json::Number::from_f64(...).unwrap()` calls from `theme.rs`. The new finite-number theme
    helper preserves `curveOpacity = 0.5` and `graticuleOpacity = 0.3`; the stale COSE-Bilkent
    y-force triage note was also removed because the diagnostic panic lives under `#[cfg(test)]`.
  - Continue HPD-080 only when a failing renderability gate, source-backed emitted-surface gap, or
    concrete consumer report points to a real blank/hidden/miscolored output defect. Otherwise,
    return to HPD-050 source-backed Architecture/Dagre/Graphlib audits instead of speculative CSS
    or raster work.
