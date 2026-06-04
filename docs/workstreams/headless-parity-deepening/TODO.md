# Headless Parity Deepening - TODO

Status: Active
Last updated: 2026-06-04

## M0 - Lane Freeze And Prioritization

- [x] HPD-010 [owner=planner] [deps=none] [scope=docs/workstreams/headless-parity-deepening,docs/quality/ARCHITECTURE_ISSUES_2026-06-01.md,docs/alignment/STATUS.md,docs/workstreams/mermaid-11-15-root-viewport-residuals]
  Goal: Freeze the next-phase lane shape, priorities, non-goals, and architecture mapping after
  Mermaid 11.15 structural parity completion.
  Validation: DESIGN.md, TODO.md, TASKS.jsonl, CAMPAIGNS.jsonl, WORKSTREAM.json, HANDOFF.md, and
  CONTEXT.jsonl exist and agree.
  Review: Confirm this lane is a deepening/refactor + residual-governance lane, not an undifferentiated
  catch-all parity backlog.
  Evidence: docs/workstreams/headless-parity-deepening/DESIGN.md
  Context: docs/workstreams/headless-parity-deepening/CONTEXT.jsonl
  Handoff: DONE. This lane now explicitly maps to the 2026-06-01 architecture audit, records the
  honest residual front after Mermaid 11.15 structural parity, and freezes a three-layer execution
  order: truth/governance first, shared seams second, deep audits and family promotion last. The
  first executable implementation slices remain HPD-020 baseline registry and HPD-030 residual
  taxonomy.

## M1 - Baseline Registry

- [x] HPD-020 [owner=codex] [deps=HPD-010] [scope=docs,crates/merman-core/src/detect,crates/merman-core/src/diagram,crates/xtask,generated provenance]
  Goal: Deepen a single Mermaid baseline registry/provenance seam so active baseline facts no
  longer drift across docs, generated names, report labels, and registry constructors.
  Validation: focused rust/doc checks plus a documented inventory diff proving baseline facts now
  project from one source.
  Review: Historical file suffixes may remain temporarily, but production call sites should stop
  presenting 11.12-era names as live baseline truth.
  Evidence: docs/workstreams/headless-parity-deepening/EVIDENCE_AND_GATES.md
  Context: docs/workstreams/headless-parity-deepening/CONTEXT.jsonl
  Handoff: DONE. `crates/merman-core/src/baseline.rs` is now the explicit baseline truth seam for
  the pinned Mermaid tag/version and the legacy generated suffix. Engine defaults, diagram/detector
  registry constructors, importer call sites, bench call sites, and xtask baseline-label reporting
  now route through the pinned-baseline path instead of presenting `default_mermaid_11_12_2*` as
  the live baseline surface. The old `default_mermaid_11_12_2*` constructors remain only as
  deprecated compatibility aliases while generated file suffixes are still historical.

## M2 - Residual Governance

- [x] HPD-030 [owner=codex] [deps=HPD-010] [scope=docs/workstreams/mermaid-11-15-root-viewport-residuals,docs/alignment,xtask reports]
  Goal: Create and apply a residual taxonomy that separates source-backed behavior gaps,
  measurement approximations, browser bbox/lattice tails, solver/phase residuals, stale baselines,
  and out-of-scope families.
  Validation: top active residual buckets are classified in workstream docs and the taxonomy is
  referenced from active residual evidence.
  Review: Do not present approximate counts as false completion metrics.
  Evidence: docs/workstreams/headless-parity-deepening/EVIDENCE_AND_GATES.md
  Context: docs/workstreams/headless-parity-deepening/CONTEXT.jsonl
  Handoff: DONE. The lane now has a durable residual taxonomy in
  `headless-parity-deepening/DESIGN.md`, and the active `mermaid-11-15-root-viewport-residuals`
  lane maps current buckets to that taxonomy. This freezes the queue-shaping model for HPD-040 and
  HPD-050 without pretending the counts are completion percentages.

## M3 - Measurement / Root Bounds Platform

- [x] HPD-040 [owner=codex] [deps=HPD-020,HPD-030] [scope=crates/merman-render/src/text,crates/merman-render/src/sequence,crates/merman-render/src/svg/parity,crates/merman-render/src/architecture_metrics.rs]
  Goal: Extract reusable measurement/root-bounds seams so future parity work stops embedding hidden
  browser-like bbox heuristics across diagram code.
  Validation: focused renderer tests for at least Sequence + Architecture plus no-growth review of
  new ad hoc constants.
  Review: The result should reduce duplication and make residuals easier to classify; it should not
  centralize unrelated diagram behavior behind one giant abstraction.
  Evidence: docs/workstreams/headless-parity-deepening/EVIDENCE_AND_GATES.md
  Context: docs/workstreams/headless-parity-deepening/CONTEXT.jsonl
  Handoff: DONE. `svg_emitted_bounds` now lives at `svg/parity` instead of under State, because
  Architecture/GitGraph/State all consume it as root-bounds infrastructure. Sequence note final
  wrap/measure semantics now flow through shared helpers used by layout, root-bounds, and render.
  No new constants or overrides were added. The known leftOf long-note root-width residual remains
  explicitly open (`570px` deterministic local vs. `566px` upstream), so this is a platform seam
  completion, not a forced root parity closure.

## M4 - Layout Engine Audit

- [ ] HPD-050 [owner=codex] [deps=HPD-030,HPD-040] [scope=crates/manatee,crates/merman-render/src/architecture.rs,docs/workstreams/mermaid-11-15-root-viewport-residuals]
  Goal: Audit `manatee` / `dugong`-adjacent layout behavior through source-backed input-model seams
  rather than broad solver rewrites, starting with Architecture residual families.
  Validation: focused Architecture residual evidence and documented outcomes per audited seam:
  source gap, input-model mismatch, or bounded browser/headless residual.
  Review: Do not start a from-scratch solver fork; the audit must stay residual-driven.
  Evidence: docs/workstreams/headless-parity-deepening/EVIDENCE_AND_GATES.md
  Context: docs/workstreams/headless-parity-deepening/CONTEXT.jsonl
  Handoff: This task may split child work if one audited seam becomes a large implementation lane.
  The current candidate split is Architecture Cytoscape bbox phase modeling: leaf default
  `node.boundingBox()`, child `updateCompoundBounds()` contribution, final group
  `node.boundingBox()`, and manatee relocation bbox currently cannot be represented by one global
  label/padding formula without broad root regressions. The dugong-adjacent source audit has also
  begun: `repo-ref/dagre` and `repo-ref/graphlib` are now checked out to their pinned commits,
  `dugong-graphlib` has a Graphlib coverage ledger, exposed Graphlib helper algorithm tests are
  ported, and the Dagre reference harness is executable again against the installed Mermaid
  `11.15.0` / `dagre-d3-es@7.0.14` toolchain. The next useful dugong slice is public Graphlib
  `Graph` API coverage, not unused shortest-path algorithms. The first public Graph API slice is
  now underway: basic options/node/edge/compound behavior is covered and parent-cycle assignment is
  guarded, while non-compound `setParent(...)` throw semantics remain an explicit open API-shape
  decision. The next Graph API slice also covers source-backed edge/adjacency queries (`sinks`,
  predecessor/successor/neighbor queries, `isLeaf`, in/out/node edge filters, and remove-edge
  neighbor updates), while preserving the open missing-node and chainable-mutator API-shape
  differences instead of forcing JS ergonomics into Rust. A follow-up edge-invariant slice now
  tightens simple-graph named-edge behavior to match upstream Graphlib: setting a named edge on a
  non-multigraph panics, and named queries/removals no longer alias the unnamed edge. The next
  public Graph API slice now covers `filterNodes` subgraph copying/compound parent promotion plus
  endpoint-aware default node/edge label callbacks without adding broad Clone bounds to ordinary
  layout graphs. A follow-up child/root API slice now adds `children_opt(...)` for source-backed
  missing-node versus empty-children semantics while keeping existing `children(...)` and
  `children_root()` behavior stable for Rust callers. A follow-up `setPath(nodes, value)` slice
  now adds `set_path_with_label(...)` with method-scoped `E: Clone` instead of forcing cloneability
  onto ordinary graph use. A follow-up `setNodes(nodes, value)` slice now adds `set_nodes(...)`
  and `set_nodes_with_label(...)` with method-scoped `N: Clone` for the batch-label API. A follow-up
  parent/clear-parent coverage slice maps Graphlib `parent(v)` and clear-parent state behavior to
  existing Rust APIs without adding JS optional-argument overloading. A follow-up `setEdge`
  coverage slice maps explicit JS `undefined` edge-label clearing to Rust `Option<T>` labels and
  edge-object parameters to `EdgeKey`, without adding JS argument overloading. A follow-up
  stringified-id boundary slice records JS numeric/object id coercion as a Rust API-shape
  non-target while covering the post-coercion undirected string-order rule for `"9"` / `"10"`
  endpoints. A follow-up
  Graphlib JSON seam slice now exposes `dugong_graphlib::json::{write, read}` with direct coverage
  for all six upstream `json-test.js` cases; the primary seam uses `Option<T>` labels to preserve
  upstream `undefined` versus explicit `null`, while default-collapsing helpers remain an explicit
  Rust bridge. The active Dagre reference adapter now reuses that Graphlib JSON shape for reference
  input, JS output, and Rust output artifacts, with the State `basic` comparison still zero-delta.
  Reuse it before adding another Graphlib-shaped serializer. ARCH-022's first
  Dagre reference adapter slice is also landed:
  `dagre_reference.rs` now owns the Rust-side
  reference input/output schema, JS harness invocation, compound-edge normalization, and Rust/JS
  delta extraction, while `compare-dagre-layout` remains a State-only graph producer. Basic,
  composite, and internal-cluster State comparisons all stayed zero-delta.
  A follow-up hardening slice now reports Rust-only and JS-only node/edge identity drift separately
  from geometry deltas, and treats JS entries that exist without coordinates/points as infinite
  diagnostic deltas. The State `basic` reference check still reports zero geometry delta and zero
  identity drift. A follow-up graph-dimension comparison slice now also reports absolute JS/Rust
  top-level graph `width` / `height` deltas from Graphlib JSON `value.width` / `value.height`; the
  State `basic` reference check reports zero graph-dimension, geometry, and identity drift.
  Architecture Cytoscape
  service-label measurement now has a shared `ArchitectureCytoscapeChildLabelBounds` seam used by
  both FCoSE node `BoundsExtras` and SVG root/group service-bounds estimation. That seam keeps SVG
  root `createText(...)` measurement separate from Cytoscape compound-child label measurement and
  preserved root residual behavior through the later behavior-preserving child-label bounds cleanup.
  A disconnected-islands root-bounds audit then
  rejected a tempting global top-level-service switch from `svg_root_bounds` to
  the Cytoscape child union: it made that one height-only row exact but expanded full
  Architecture root mismatches from `26` to `84`. The first narrow phase-specific follow-up is now
  landed: isolated top-level services in diagrams that also have groups use the Cytoscape
  group-child phase for root contribution, making the disconnected-islands row exact and reducing
  Architecture root mismatches from `26` to `25` while preserving structural parity.
  Fresh 2026-06-03 classification confirms the active Architecture root queue is `25` rows. Do not
  reopen the now-exact `batch4_init_small_icons`, `batch4_init_fontsize_wrap`,
  `edge_label_corner_cases`, `fan_in_out`, `deep_nesting`,
  `batch6_junctions_multi_split_with_group_edges`, or `disconnected_islands` rows. Continue from
  source-backed audits of `junction_fork_join`, the `+5px` group/service bbox rows, the `unicode`
  / `nested_groups` compound-bounds rows, and `group_port_edges`. A later child-bbox/source-phase
  experiment confirmed the first-principles gate policy: raw Cytoscape label/body/final group
  formulas improved the two `+5px` rows locally but expanded full Architecture root mismatches
  from `25` to `100`, so production changes were reverted and `parity-root` remains diagnostic
  unless a seam survives family-level verification. A follow-up child-label bounds cleanup made the
  Cytoscape child-label bounds phase explicit in code without changing Architecture output:
  structural parity stayed green and `parity-root` remained the existing `25` mismatch diagnostic
  queue.
  A follow-up child-contribution bounds seam then replaced the remaining single
  `cytoscape_group_child_bounds` code field with
  `ArchitectureCytoscapeChildContributionBounds { body_bounds, label_bounds, union_bounds }`.
  SVG/group service-bounds estimation and isolated top-level service root-bounds logic now consume
  the explicit `union_bounds`; Architecture structural parity stayed green and `parity-root`
  remained the existing `25` mismatch diagnostic queue.
  A follow-up FCoSE contribution seam then routed
  `architecture_measure_cytoscape_node_bbox_extras(...)` through the same expanded-body,
  optional-label, and union contribution vocabulary before deriving `BoundsExtras`. This preserved
  Architecture structural parity and left the root queue at the existing `25` diagnostics.
  A follow-up probe-harness slice then promoted the manual Architecture FCoSE/Cytoscape browser
  probe into `xtask debug-architecture-fcose-probe`, with fixture resolution, JSON validation,
  stable artifact naming, and an optional `--browser-exe` bridge for the existing Edge-backed
  Puppeteer workflow. This is source-evidence infrastructure only; it did not change layout or
  root-bounds behavior.
  A follow-up probe-summary slice now writes a Markdown table beside the raw JSON, exposing config,
  `bbBeforeRun2` / `bbAfterSegments`, final `node.boundingBox()`, `bodyBounds`,
  `labelBounds.all`, and children bbox phases. The two active `+5px` group/service bbox rows
  (`batch5_long_titles_and_punct_076` and `html_titles_and_escapes_041`) both generated summaries;
  continue using these artifacts before attempting another source-formula change. A follow-up
  expansion-summary slice adds `bb over children labels` to that final-node table, making final
  group bbox expansion over `childrenBoundingBoxIncludeLabels` explicit as left/right/top/bottom
  plus `dw` / `dh`. A follow-up active-residual batch regenerated the seven representative
  Architecture probe summaries with that column under
  `target\compare\architecture-fcose-probe-expansion-active-residuals-hpd050\`; use that batch
  before any next group-bbox formula experiment.
  A follow-up label-contribution summary slice adds `children labels over body` to the same final
  node table and regenerated the seven active-residual probes under
  `target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050\`. The table
  now exposes `children body -> children labels -> final node.boundingBox()` in one row, so future
  group-bbox formula work can separate child label contribution from final compound expansion
  without manual subtraction. This remains evidence tooling only.
  A follow-up label-phase join regenerated current-HEAD local deltas under
  `target\compare\architecture-delta-label-phase-current-hpd050\` and joined them with that probe
  batch. `group_port_edges_017` is now zero-delta on current HEAD and should not be reopened from
  stale pre-Procrustes artifacts. The remaining direct group-width tails are `batch5` `+5px`,
  `html_titles` `+5px`, and `unicode` `+3px`, but the joined child-label and final-group phases
  reject another standalone group-padding, font-family switch, or exact labelWidth lookup attempt.
  A follow-up group-content-union source audit narrowed those direct width tails to local child
  service-label/content bounds feeding `GroupRectComputer`. Pinned Mermaid draws group rects from
  final Cytoscape `node.boundingBox()`, while local rebuilds rects from service/junction/child
  group bounds and then applies `padding + 2.5px`; focused debug runs show `batch5`, `html_titles`,
  and `unicode` are already too wide in that child content union phase. Do not change group
  padding, root padding, group title bounds, or final rect emission for these rows.
  A follow-up local FCoSE compound-bounds output slice now exposes manatee's final layout-base
  compound rectangles as `ArchitectureDiagramLayout.fcose_compound_bounds` and adds a
  `debug-architecture-delta` table comparing those rects to the local emitted SVG group rects.
  This confirms the local FCoSE rect phase is materially different from emitted group rects:
  `pipeline`, `ui`, and `i` are `+107px`, `+44px`, and `+32px` wider in emitted space than in
  FCoSE layout-base space, while their upstream/local emitted width tails remain only `+5px`,
  `+5px`, and `+3px`. Keep the new field as evidence only; do not wire it into group rendering as
  a shortcut.
  A follow-up service-contribution report slice now exposes
  `ArchitectureDiagramLayout.cytoscape_service_bounds` and adds a stable Markdown table for local
  service body/label/union phases in `debug-architecture-delta`. The direct width-tail reports now
  show child inputs such as `batch5/storage=225x97`, `html_titles/web=129x97`, and
  `unicode/metrics=125x97` without relying on stderr-only `MERMAN_ARCH_DEBUG_GROUP_RECT` output.
  Keep this as a child-contribution evidence surface, not a generic root-bounds source.
  A follow-up service phase-join slice used that table beside the browser/Cytoscape final-node
  probe to decompose the direct width tails: `batch5/pipeline` and `html_titles/ui` are
  `content dw=+3` plus `expansion dw=+2`, while `unicode/i` is `content dw=+1` plus
  `expansion dw=+2`. The height side has the opposite content/expansion split (`content dh=-2`,
  `expansion dh=+2`), so group padding still cannot be changed alone without regressing height.
  Continue from individual service label/content union width versus browser final service bbox.
  A follow-up probe phase-join automation slice adds optional `--probe-dir` support to
  `debug-architecture-delta`, reading the matching browser probe JSON and emitting the group
  content decomposition plus service bbox join directly in each local delta report. The generated
  reports under `target\compare\architecture-delta-probe-phase-join-hpd050\` reproduce the same
  `+3/+3/+1` content width split, stable `+2` final expansion split, and height-side `-2/+2`
  cancellation without manual table subtraction. Treat this as evidence tooling only; the next
  production seam remains individual service label/content contribution width plus service
  position drift.
  A follow-up service-label-metrics slice adds local label `text_width`, `half_width`, and
  `applied_scale` to `ArchitectureCytoscapeServiceBounds`, then joins those values with browser
  final-node `metrics.labelWidth` / `metrics.labelHeight` in `debug-architecture-delta --probe-dir`.
  The focused reports under `target\compare\architecture-delta-service-label-metrics-hpd050\` show
  this is not a one-constant drift: `storage` has raw metric `dw=+5.828` and contribution-label
  `dw=+4`, `web` has raw metric `dw=-0.430` but contribution-label `dw=+2`, and `metrics` has raw
  metric `dw=+1.055` but contribution-label `dw=+4`. Continue from a phase-specific service
  final-bbox contribution model; do not try a global label-scale, body-border, group-padding, or
  final-rect tweak.
  A follow-up child-union edge-attribution slice now compares browser service child union
  (`bodyBounds` union `labelBounds.all`) with local service contribution shifted into the same
  final-frame coordinates. The direct group content tails are attributed to boundary services:
  `batch5/pipeline` is `storage left dx=-2.5` plus `registry right dx=+0.5` for `edge dw=+3`,
  `html_titles/ui` is `web left dx=-0.5` plus `origin right dx=+2.5` for `edge dw=+3`, and
  `unicode/i` is `metrics left dx=-3.5` plus `store right dx=-2.5` for `edge dw=+1`. The top/bottom
  split is stable at `+1/-1`, giving child-union `edge dh=-2` before final group expansion cancels
  it. Continue from boundary service child-contribution modeling, not aggregate group width.
  A follow-up source audit confirmed the exact upstream phase split in Mermaid `11.15.0` and
  Cytoscape `3.33.4`: compound sizing uses `children.boundingBox({ includeLabels: true,
  includeOverlays: false, useCache: false })`, which unions stored body bounds and label bounds;
  body bounds have a `1px` expansion, label bounds include hardcoded `marginOfError = 2`, and
  default final `node.boundingBox()` adds a separate whole-bbox `1px` expansion. This makes the next
  production-capable seam Architecture service `labelWidth` measurement, not body-border or
  group-padding tweaks.
  A follow-up labelWidth measurement-seam audit confirmed that the reusable infrastructure is the
  Architecture browser probe/report pipeline, not the C4 headless-shell SVG text lookup table. C4's
  table measures SVG `<text>.getBBox().width`; Architecture needs Cytoscape renderer
  `metrics.labelWidth` for compound child sizing. The active probe batch already contains service
  labelWidth evidence for the current residual set, and `debug-architecture-delta --probe-dir`
  joins it with local service label metrics. Do not add an Architecture lookup-only production patch
  unless it is paired with the source child-union/final-bbox phase model and full Architecture root
  verification.
  A follow-up service-final-bbox report slice adds `local final bb final-frame` and final
  `dx/dy/dw/dh` columns to `debug-architecture-delta --probe-dir`. The source-shaped `1px` final
  `node.boundingBox()` expansion leaves boundary-service width drift in the child contribution
  phase (`registry +2`, `storage +4`, `web +2`, `origin +4`, `metrics +4`, `store -2`) and reduces
  the height comparison to a stable service final-bbox `-1px`. Treat this as evidence tooling only,
  not as a final rect or group-padding production fix.
  A follow-up service-label final-frame report slice adds
  `local contribution label final-frame` plus label `dx/dy/dw/dh` columns to the same service join.
  The focused rows show a stable `label dy=-78` / `label dh=+77` because local contribution-label
  bounds are extended child contribution rectangles from icon top to label bottom, not browser
  text-label bounds. The actionable signal remains the service-specific horizontal drift
  (`registry +2`, `storage +4`, `web +2`, `origin +4`, `metrics +4`, `store -2` label `dw`) plus
  placement drift; do not treat the vertical label comparison as a production bug.
  A follow-up current residual ordering slice adds `max-width delta` to
  `summarize-architecture-deltas` and sorts by absolute max-width residual before fixture name. A
  fresh current Architecture `parity-root` report expected-fails with `24` root-only mismatches, and
  the regenerated summary now puts `junction_fork_join`, `batch5`, `html_titles`, `unicode`,
  `batch6_init`, and `nested_groups` at the top. `group_port_edges_017` is zero-delta on current
  HEAD and should not be treated as active unless a fresh report regresses.
  A follow-up root-score summary slice extends that report with viewBox width delta, viewBox height
  delta, and `root residual score`, computed as the max absolute residual across `max-width`,
  viewBox width, and viewBox height. The regenerated
  `target\compare\architecture-delta-summary-root-score-hpd050\architecture-delta-summary.md`
  keeps the same active top queue while also ordering smaller height/viewBox tails honestly; for
  example, `group_to_group_multi_034` now ranks by its `0.755px` height score above
  `long_group_titles_018` at `0.656px`. This is report governance only, not renderer or root-bounds
  tuning.
  A follow-up delta-batch root-score slice projects the same vocabulary into
  `debug-architecture-delta`: per-fixture reports now list viewBox width/height delta,
  max-width delta, and root residual score, and multi-fixture
  `architecture-delta-batch.md` sorts by that score. The regenerated current-top batch is
  `target\compare\architecture-delta-current-top-root-score-hpd050\architecture-delta-batch.md`;
  use it as the first local-delta entrypoint for the active Architecture residual set.
  A follow-up nested aggregate-edge slice adds `Group aggregate edge attribution` to the same
  probe-backed delta reports. The regenerated
  `target\compare\architecture-delta-current-top-aggregate-edge-hpd050\architecture-delta-batch.md`
  shows `nested_groups_002/platform` as child groups `data, runtime`, with width owned by `data`
  on both left/right edges (`left dx=44.25`, `right dx=43.75`, `edge dw=-0.5`) and height balanced
  between `runtime` top and `data` bottom (`edge dh=0`). Continue treating this as child-group
  aggregate boundary drift, not direct-service or final-expansion evidence.
  A follow-up production-path experiment rejected retuning `GroupRectComputer`'s global
  `child_group_inset` from `1.0` to `0.75`: Architecture `parity-root` expanded from `24` to `44`
  mismatches, `nested_groups_002` worsened to `+2.75`, and `group_port_edges_017` regressed back
  into the queue. The code was restored; do not pursue global child-group inset tuning as the
  nested residual fix.
  A follow-up render-path probe slice adds
  `tools/debug/arch_render_path_probe_fixture.js`, which patches the installed Mermaid `11.15.0`
  IIFE in memory and runs `mermaid.render(...)` instead of manually rebuilding ArchitectureDB /
  Cytoscape inputs. For `junction_fork_join_026`, it reproduces the stored upstream SVG facts
  exactly and shows the final SVG emission consumes the post-rerun `cy-ready` state
  (`left=1788.557x1649.154`), not the first layoutstop state
  (`left=1805.888x1630.544`). Treat the manual FCoSE probe as diagnostic-only when it disagrees
  with this render-path evidence; future junction work should target bundled FCoSE/internal phases,
  not stored-baseline drift or a one-off manatee tune.
  A follow-up xtask wrapper slice promotes that real render-path probe into
  `debug-architecture-render-path-probe`, with repeated `--fixture`, stable
  `.render-path-probe.json` / `.render-path-probe.md` artifacts, optional `--browser-exe`, and a
  batch index. Use this wrapper before any future `junction_fork_join_026` source claim so the
  evidence stays tied to `mermaid.render(...)` rather than hand-run Node output.
  A follow-up delta batch CLI slice lets `debug-architecture-delta` accept repeated `--fixture`
  filters, including `--probe-dir` joins. Use this to regenerate the focused Architecture residual
  reports in one command before any next source-backed formula experiment, instead of relying on
  stale per-fixture artifacts or hand-written shell loops.
  A follow-up local-delta batch-index slice writes `architecture-delta-batch.md` beside multi-fixture
  delta outputs. The index lists each fixture's report, copied upstream/local SVGs, optional probe
  JSON, `max-width` delta, and matched element counts. Use
  `target\compare\architecture-delta-batch-index-hpd050\architecture-delta-batch.md` as the first
  entrypoint for the current `batch5` / `html_titles` / `unicode` local-vs-browser service
  contribution reports.
  A follow-up nested-group aggregate slice adds a `Group aggregate child attribution` table to
  `debug-architecture-delta --probe-dir`. The table combines local direct service contribution
  bounds with direct child-group emitted rects before comparing them to browser
  `childrenBoundingBoxIncludeLabels`. The current top-residual batch at
  `target\compare\architecture-delta-current-top-residuals-hpd050\architecture-delta-batch.md`
  now shows `nested_groups_002/platform` as child groups `data, runtime`, `content dw=-0.500000`,
  and expansion `dw=0`, so nested parent residuals no longer depend on a `<none>` direct-service
  blind spot.
  A follow-up edge-summary slice adds final edge rows to the same Markdown output. The focused
  `group_port_edges_017` probe now records browser/Cytoscape edge bboxes, endpoint coordinates,
  source/target directions, and segment style values as table evidence; keep using this for
  edge/endpoint residual triage instead of reopening renderer routing from SVG shape alone.
  A follow-up batch-probe slice allows repeated `--fixture` flags, so the active Architecture
  residual samples can be captured in one command while still writing per-fixture JSON/Markdown
  artifacts. Use this batch mode for future small residual classes before changing production
  layout formulas.
  A follow-up batch-index slice writes `architecture-fcose-probe-batch.md` beside batch outputs,
  listing each fixture's JSON/Markdown artifact and captured stage/node/edge counts. Use the index
  as the first entrypoint when citing multi-fixture source-backed probe evidence.
  A follow-up active-residual batch used that index flow against the current 25-row Architecture
  root queue and captured seven representative samples in one run:
  `junction_fork_join_026`, `batch5_long_titles_and_punct_076`,
  `html_titles_and_escapes_041`, `unicode_and_xml_escapes_019`, `nested_groups_002`,
  `batch6_init_fontsize_icon_size_wrap_093`, and `group_port_edges_017`. The batch index is
  `target\compare\architecture-fcose-probe-active-residuals-hpd050\architecture-fcose-probe-batch.md`.
  This is evidence collection only; use the per-fixture summaries for source-backed phase
  comparison before changing root-bounds or layout formulas.
  A follow-up local-delta evidence repair then fixed `debug-architecture-delta` /
  `summarize-architecture-deltas` to recognize current diagram-scoped Architecture SVG ids
  (`<diagram>-service-*`, `<diagram>-group-*`, and junction child `<diagram>-node-*`). The seven
  active residual local reports now capture service, junction, and group-rect deltas instead of
  root-only data; use
  `target\compare\architecture-delta-active-residuals-hpd050\*.md` beside the browser probe batch
  before making any production phase/formula change.
  A follow-up local-delta report slice then added explicit group `dw` / `dh` columns plus group max
  delta columns in `summarize-architecture-deltas`. The regenerated reports under
  `target\compare\architecture-delta-active-residuals-hpd050-group-size\` now show, for example,
  the two `+5px` rows as direct group `dw=+5.000px` tails and `group_port_edges_017` as
  `group-outer dh=-17.845px`. This keeps the next comparison source-backed instead of relying on
  manual parsing of `x/y/w/h` strings.
  A follow-up phase-join pass then compared those local group `dw` / `dh` rows against the browser
  probe final group bboxes. Non-junction rows now have a direct final-bbox-to-upstream-group-width
  join, while `junction_fork_join_026` is explicitly marked as probe-vs-baseline divergence before
  it can drive a formula change. `group_port_edges_017` is the clearest next phase seam: local
  outer group height equals browser `bbAfterSegments.h=444.603px`, but upstream final outer group
  height is `462.448px`, producing the `-17.845px` root-height tail.
  A follow-up source audit then confirmed why that row should not be fixed with a group-padding
  tweak. Local SVG group rectangles are rebuilt by `GroupRectComputer` from leaf/child bounds,
  while pinned Mermaid draws group rectangles from Cytoscape final `node.boundingBox()`. A focused
  `MANATEE_FCOSE_DEBUG_ELES_BBOX=1` run showed the local `run=1` element bbox height is
  `444.603px`, matching browser `bbAfterSegments` and the local outer group height; the stored
  upstream SVG outer group remains the later final compound bbox phase at `462.448px`. The local
  service/inner-group positions are also vertically compressed by `8.922571px` on each side, so the
  next production path must separate layout relocation bboxes, final compound group bboxes, and
  `{group}` endpoint propagation rather than exporting layout-base compound rects directly.
  A follow-up relocation/repulsion audit narrowed that implementation path further. Disabling
  relocation made `group_port_edges_017` root-exact, but it increased full Architecture
  `parity-root` mismatches from `25` to `27`, so relocation is not a global switch fix. Enhanced
  browser probe evidence shows first-run relocation matches local exactly, and second-run
  `originalCenter` also matches at `(1.500,17.750)`. The divergence starts in the second run's
  first CoSE tick before constraint relaxation: upstream gives the `inner` compound
  `repulsion=(0,250)` / displacement `(0,30)`, while local gives `repulsion=(40,40)` /
  displacement `(6,6)`. Current evidence points to a `layout-base` clipping / near-touching
  rectangle boundary after `ConstraintHandler.handleConstraints(...)`, not renderer group padding
  or a wrong `eles.boundingBox()` original-center input. Do not globally change
  `rects_intersect(...)` or add an epsilon without a focused clipping parity test and full
  Architecture verification.
  A narrow Architecture Procrustes compatibility slice is now landed: `group_port_edges_017`
  returns to the upstream root viewBox at 3-decimal precision, the Architecture root mismatch
  queue drops from `25` to `24`, and structural parity stays green. Treat the remaining root
  rows as diagnostic until a separate source-backed seam justifies more work.

## M5 - Semantic / Render Unification Pilot

- [x] HPD-060 [owner=codex] [deps=HPD-020,HPD-040] [scope=crates/merman-core,crates/merman-render]
  Goal: Pick one high-value diagram family and deepen the “one semantic truth, adapters on top”
  direction to reduce typed-vs-JSON parallel master paths.
  Validation: pilot design note + focused implementation slice or an explicit split decision with
  narrowed scope.
  Review: This is a seam-deepening pilot, not a full repo-wide migration in one task.
  Evidence: docs/workstreams/headless-parity-deepening/EVIDENCE_AND_GATES.md
  Context: docs/workstreams/headless-parity-deepening/CONTEXT.jsonl
  Handoff: DONE. Sequence was selected as the bounded pilot. `SequenceDb::into_model(...)` now
  delegates through `into_render_model().to_compat_json(...)`, so compatibility JSON is projected
  from the typed render model instead of a second DB-side manual JSON master. The focused test now
  covers messages, notes, boxes, create/destroy indexes, and omitted message fields. Sequence
  structural SVG parity remains green; existing Sequence root-measurement residuals remain open.

## M6 - New Family Rubric

- [x] HPD-070 [owner=planner] [deps=HPD-010,HPD-030] [scope=docs/alignment,repo-ref/mermaid/packages/mermaid/src/diagrams]
  Goal: Define a durable rubric and priority order for unsupported Mermaid families so new scope is
  promoted intentionally and only when it fits the headless architecture.
  Validation: rubric and priority list documented; `treeView`, `venn`, `ishikawa`, `eventmodeling`,
  `wardley`, `railroad-*`, and `cynefin` are classified.
  Review: Do not silently imply these families are “next up” without a capability fit argument.
  Evidence: docs/workstreams/headless-parity-deepening/EVIDENCE_AND_GATES.md
  Context: docs/workstreams/headless-parity-deepening/CONTEXT.jsonl
  Handoff: DONE. Added `docs/alignment/UNSUPPORTED_FAMILY_ADMISSION_RUBRIC.md` and corrected the
  unsupported-family table in `docs/alignment/STATUS.md` against the locked Mermaid 11.15 commit.
  New-family priority is `treeView`, `ishikawa`, `eventmodeling`, `venn`, then `wardley`.
  `railroad-*` and `cynefin-beta` are absent from the pinned 11.15 source and should not be treated
  as 11.15 backlog items.

## M7 - Visible Rendering Defect Triage

- [ ] HPD-080 [owner=codex] [deps=HPD-030] [scope=crates/merman-render/src/svg/parity,fixtures,xtask compare commands]
  Goal: Find and fix functional rendering defects that DOM structural parity can miss, especially
  missing Mermaid 11.15 diagram-specific CSS/theme emission that makes output unreadable or loses
  semantic color cues.
  Validation: source-backed style/rendering checks against pinned Mermaid 11.15 plus focused
  renderer tests and compare commands for touched diagram families.
  Review: This task outranks small numeric `parity-root` residuals, but it is not a license for
  cosmetic overfitting. Fix only defects backed by Mermaid source, official fixtures, or direct
  user-visible breakage.
  Evidence: docs/workstreams/headless-parity-deepening/EVIDENCE_AND_GATES.md
  Context: docs/workstreams/headless-parity-deepening/CONTEXT.jsonl
  Handoff: IN_PROGRESS. The first source-backed slice fixed missing or incomplete theme CSS emission
  for Kanban, Packet, Sankey, C4, and GitGraph. The user Kanban metadata and GitGraph merge samples
  now render with readable labels and semantic branch/card styling. Follow-up slices fixed Gantt,
  Treemap, Requirement, Mindmap, Pie, and Journey theme/readability gaps, including Mindmap XHTML
  `span` label colors, Pie title/slice/legend text variables, and Journey task/section fillType
  rules. A later ER slice fixed stale default-theme CSS for entity boxes, labels, relationship
  lines, markers, and edge-label backgrounds while explicitly skipping inert upstream rules that
  require local SVG attributes we do not emit yet. A Radar slice fixed top-level `radar.*` style
  overrides that were parsed but not emitted into CSS. A Block slice fixed composite cluster fade
  CSS for nested block readability. A Sequence slice fixed stale hardcoded actor, lifeline, signal,
  label, note, activation, marker/error, and rect node CSS by routing Sequence styles through
  `effective_config` and the shared `SvgTheme` seam. A State slice fixed stale hardcoded state
  node, cluster, transition, label, note, marker, start/end, special-state, and title CSS, including
  the prefixed local barbEnd marker selector. A Flowchart slice fixed hardcoded node/edge-path
  stroke widths so Mermaid 11.15 numeric `themeVariables.strokeWidth` controls visible stroke
  thickness. A Class namespace slice restored Mermaid 11.15 namespace-qualified relation facade
  semantics in core, while ASCII now folds only empty facade classes back to their declared
  namespace members as a view-layer alias to avoid duplicate terminal boxes. A Timeline slice fixed
  `.disabled` CSS so `themeVariables.tertiaryColor` and `clusterBorder` drive disabled node/text
  colors instead of stale hardcoded fallback fills. An Architecture slice fixed source-backed
  `archEdgeColor`, `archEdgeArrowColor`, `archEdgeWidth`, `archGroupBorderColor`, and
  `archGroupBorderWidth` emission so custom Architecture edge/group styling reaches the final SVG
  stylesheet instead of falling back to generic line/group-border colors. A Class note slice fixed
  `noteBkgColor` and `noteBorderColor` for both HTML-label and `htmlLabels:false` note shapes,
  complementing the existing `noteTextColor` CSS coverage. A follow-up Class stylesheet slice
  restored source-backed node shape, divider, cluster, class-label, edge-terminal, and relation CSS
  rules that apply to current output, including numeric `themeVariables.strokeWidth` via the shared
  CSS-token path. A Zed integration feedback slice added an optional
  `DropNativeDuplicateFallbacksPostprocessor` so `resvg_safe` consumers can drop only fallback
  labels that duplicate native SVG text without losing fallback-only labels. A theme coverage ledger
  now records implemented-matrix style-provider coverage, deferred inert rules, and host theme
  boundaries so future HPD-080 work does not fake CSS parity or copy Zed-specific palette policy
  into default output. A XYChart coverage slice now proves Mermaid 11.15 inline `xyChart` theme
  values reach the final SVG render path without inventing a non-existent CSS provider. A
  cross-diagram dark-theme smoke slice then fixed Flowchart `nodeTextColor` CSS emission and locked
  the public render API against representative readable theme signals for Flowchart, Sequence,
  Kanban, GitGraph, and XYChart while preserving upstream Kanban placeholder-class behavior. A
  QuadrantChart follow-up fixed invalid default data-point colors: Mermaid 11.15's shipped theme
  expansion emits `hsl(...NaN%)` because khroma `lighten`/`darken` are called without an amount,
  while merman now emits a valid derived default and keeps valid explicit `quadrantPointFill`
  overrides. A follow-up raw-SVG cleanup then removed useless upstream
  `style="undefined;;;undefined"` artifacts from ER relationship paths and Mindmap edge paths
  while preserving class-driven edge styling and structural parity. A Mermaid 11.15 theme-surface
  correction then exposed all 11 official theme names through core, bindings, and `@merman/web`
  while keeping exact `neo/redux*` override derivation as an honest follow-up. A binding options
  slice then exposed `svg.drop_native_duplicate_fallbacks` so non-Rust hosts can opt into the same
  duplicate native/fallback cleanup as Rust `SvgPipeline` consumers without changing default
  `resvg_safe` behavior. A follow-up binding options slice added top-level `site_config`, so
  non-Rust hosts can pass Mermaid theme, `themeVariables`, diagram config, and `themeCSS` defaults
  without injecting init directives into source text. A follow-up binding options slice then added
  explicit host-owned `svg.scoped_css` and `svg.css_override_policy`; `resvg-safe` binding pipelines
  sanitize injected host CSS after insertion. A root-background source/capture audit confirmed that
  Mermaid 11.15 `setupGraphViewbox` does not emit root `background-color`, while local parity output
  preserves the capture-compatible white background. Rust hosts can now use
  `RootBackgroundPostprocessor`, and binding hosts can use `svg.root_background_color` to change the
  root canvas explicitly, while default Mermaid parity output stays unchanged. A source-checkout
  audit then restored `repo-ref/mermaid` from accidental `develop` drift to the lockfile Mermaid
  `11.15.0` commit and confirmed the theme coverage ledger still matches the pinned provider
  inventory. A follow-up Zed-feedback audit reconfirmed that common host theme needs are covered
  without making Zed's exact editor palette cleanup a merman default, and used Zed PR `58325` to
  fix a serious Flowchart deep-subgraph stack-overflow risk by moving cluster tree traversals onto
  explicit stacks. A follow-up resvg-safe fixture smoke slice added a public API host-integration
  gate for the user Kanban/GitGraph examples, a dark-theme Flowchart sample, and representative
  supported-family fixtures; the gate rejects foreignObject reliance, invalid visual tokens, empty
  style elements, and, under the `raster` feature, SVGs that cannot convert to PNG. A follow-up
  all-supported resvg-safe audit resolved the Flowchart `layout.rs` conflict between the Zed PR
  `58325` backport shape and local explicit-stack traversal coverage and fixed empty Pie roots to
  emit finite headless-safe viewBoxes. A later source-backed Treemap correction reversed the earlier
  bare-label-token assumption: pinned Mermaid 11.15 renders `classDef ... color;` as an error, so
  local parsing now rejects bare style tokens instead of pretending DB-layer tolerance is parser
  parity.
  A follow-up extended-theme slice fixed a real host theme override gap in the official
  `neo/redux*` themes: defaults still come from generated Mermaid 11.15 snapshots, but user base
  overrides now recompute source-backed visible derived keys consumed by current renderers, while
  direct derived-key overrides keep winning after derivation.
  A follow-up dark extended-theme slice added source-backed `neo-dark` / `redux-dark*`
  `primaryColor` derivations for Requirement, Pie, QuadrantChart, and `redux-dark*` GitGraph
  palettes/inverses, and fixed Pie layout so slice/legend colors consume
  `themeVariables.pie1..pie12` instead of a hardcoded default palette.
  A Journey arrowhead audit then tightened the public dark-theme smoke by removing
  `arrowheadColor` from visible-signal assertions: Mermaid 11.15 emits `.arrowheadPath`, but current
  Journey marker DOM has no matching class, so counting that CSS token as visible coverage was
  self-deceptive.
  The ignored all-supported `resvg-safe` audit now supports
  `MERMAN_RESVG_SAFE_AUDIT_FAMILY` and `MERMAN_RESVG_SAFE_AUDIT_FILTER` filters after render-only
  all-supported passed but unfiltered raster all-supported timed out without fixture-level signal.
  A follow-up raster ink gate now decodes PNG output and rejects contentful diagrams that rasterize
  as blank/all-background images, while treating header-only, accessibility-only, and title-only
  metadata fixtures as non-visual. Architecture/Class/Sequence passed the focused filtered raster
  audit after that calibration.
  A follow-up raster ink calibration slice tightened the source-content detector for Journey
  section-only, `packet-beta` header-only, Radar option-only, and Treemap no-value inputs, then fixed
  a real single-leaf Treemap readability failure: transparent first-leaf fill plus white default
  `cScaleLabel0` text now falls back to `themeVariables.textColor` only when no explicit leaf fill
  override exists.
  A follow-up directive-only calibration slice then fixed false raster-ink failures for
  State `classDef`-only, State parser-only floating note declarations, and Flowchart `click`-only
  fixtures. The fix is limited to the source-content gate; renderer behavior was not changed because
  pinned Mermaid 11.15 treats these inputs as parser/metadata cases with no required visible marks.
  A boundary renderability slice then added a separate `info` / `error` / `zenuml`
  `resvg_safe` fixture smoke. These entrypoints stay out of the supported-family style-provider
  matrix, but now have the same XML, foreignObject, invalid-token, empty-style, and raster ink
  regression gate used for public renderability.
  A follow-up CI/compare diagnosis confirmed the reported cross-platform Sequence width and Class
  namespace snapshot failures were stale relative to current HEAD, then fixed two fresh structural
  rendering regressions: default-theme Pie `themeVariables` merging now preserves source-backed
  `pie1/#ECECFF` and `pie2/#ffffde` base colors under unrelated overrides, and Treemap invalid
  bare classDef style tokens now render through the suppressed error diagram like pinned upstream.
  A follow-up Journey/Timeline visible-signal audit then tightened the public dark-theme smoke so
  CSS rules without matching current DOM are no longer counted as visible renderability coverage.
  Journey no longer counts inherited Flowchart-like provider rules (`.flowchart-link`, `.edgeLabel`,
  `.edgePath .path`, `.node ...`, `.cluster text`, `.arrowheadPath`); Timeline no longer counts
  `.disabled` CSS from a compact source that emits no disabled DOM. Focused tests document both
  source-backed boundaries. A follow-up Requirement audit applied the same standard: legacy
  `.reqBox` / `.reqTitle` / `.relationshipLabel` provider rules stay tracked as provider coverage,
  but the public dark-theme smoke now counts only current DOM-consumed Requirement signals. The same
  slice fixed the actual visible gap by emitting `look: neo` Requirement node/edge DOM surfaces so
  `nodeBorder` can style current node and divider strokes while default structural parity stays
  green.
  A follow-up Gantt visible-signal audit found no production CSS defect, but the compact smoke
  source was still self-deceptive: it counted ordinary task colors while rendering only a `done`
  task, and it counted outside-label color without proving outside-label DOM. The smoke now includes
  a wide ordinary task, a narrow long-label task, and a done task so normal task, outside text, and
  done task colors each have matching DOM before being counted as visible renderability signals.
  A follow-up GitGraph official-theme audit confirmed the user-provided merge sample itself was
  readable, then found a source-backed CSS gap for Mermaid 11.15 `neo` / `redux*` themes. Local
  GitGraph now mirrors the upstream color-generation branches for redux geometry, redux color
  themes, and neo gradient label backgrounds, including scoped gradient defs for `neo` output.
  A follow-up raster integration slice fixed the Ubuntu-only blank PNG failure for the boundary
  `info` fixture without pretending `info` is metadata-only. PNG/JPEG raster options now provide
  browser-like fallback when a configured family such as `courier` is unavailable and use
  `max-width` as the default viewport width for no-`viewBox` Mermaid SVGs.
  A follow-up Sequence autonumber slice fixed the user-visible activation-bound anchor bug in the
  reported `autonumber` sample. Sequence number markers now follow Mermaid 11.15's
  `activationBounds(...)` / `fromBounds` / `toBounds` formula instead of using the message line's
  first point, so messages sent from an active participant anchor on the same side of the activation
  rectangle as upstream.
  A follow-up Sequence layout slice fixed the same full-stack activation-bounds rule for message
  endpoints. Nested activations now use the min-left / max-right across all active rectangles,
  matching Mermaid's `activationBounds(actor, actors)` instead of only the current stack top.
  A follow-up Sequence activation seam slice then centralized the Mermaid 11.15 activation start
  and full-stack bounds formulas so layout, activation-rect SVG planning, and autonumber marker
  placement cannot silently diverge again.
  A follow-up C4 visible-signal audit found no production defect, but tightened public smoke
  coverage so inert `.person` provider CSS is not counted as visible C4 coverage. C4 visible colors
  are now proven through inline `c4` config plus `UpdateElementStyle` / `UpdateRelStyle` output.
  A follow-up Packet/Sankey visible-signal audit found no production defect, but now proves their
  public theme colors only when matching Packet DOM classes, Sankey outlined-label DOM, node rect
  fills, and link groups exist.
  A follow-up Mindmap visible-signal audit found no production defect, but stopped the public smoke
  from counting root-section CSS that is overwritten by `.section-root` rules or native-text CSS
  that current XHTML label DOM does not consume.
  A follow-up ER visible-signal audit found no production defect, but stopped the public smoke from
  counting direct `.relationshipLabelBox` and native edge-label text CSS when current ER labels are
  XHTML spans.
  A follow-up State visible-signal slice then fixed a real production seam: current State ordinary,
  choice, fork/join, end, and note surfaces render as rough inline shapes, so source-backed theme
  defaults now feed final visible SVG attributes for those paths/circles instead of only emitting
  stylesheet tokens that the current DOM would not consume.
  A follow-up Flowchart visible-edge slice fixed the same class of production seam for ordinary
  edges. Pinned Mermaid 11.15's shared stylesheet drives `.edge-thickness-normal` from
  `themeVariables.strokeWidth`, and current Flowchart paths emit that class; local output had only
  updated `.edgePath .path`, which current paths do not carry. Flowchart now themes the visible
  edge class while preserving explicit `linkStyle` stroke-width overrides.
  A follow-up Block visible-edge slice fixed the same shared-rule gap for Block. Current Block edge
  paths emit `edge-thickness-normal edge-pattern-solid flowchart-link`, while local Block CSS had
  only the diagram-owned `.edgePath .path` width. Block now emits Mermaid 11.15 shared edge
  thickness/pattern classes so `themeVariables.strokeWidth` reaches visible Block edges.
  A follow-up Timeline redux visible-DOM slice fixed the official `redux*` theme branch. Current
  Timeline redux node paths now consume `mainBkg` / `nodeBorder` / `strokeWidth`, labels consume
  `nodeBorder` / `fontWeight`, and `.lineWrapper line` consumes `nodeBorder` / `strokeWidth`;
  redux node geometry also uses the source-backed sharp-corner path without the classic divider
  line. Structural Timeline `parity` stayed green, while the known `3` Timeline max-width
  `parity-root` rows remain diagnostic.
  A follow-up Mindmap look/theme seam slice fixed a real production seam: Mindmap parser data and
  typed render data now project Mermaid `look` into nodes and edges instead of hardcoding
  `"default"`, default semantic snapshots use Mermaid's configured `"classic"`, and default nodes
  use the source-backed redux shape branch. Current SVG output emits `data-look="neo"` only for
  `neo` nodes/edges, then applies the matching Mermaid 11.15 neo node/root/edge/drop-shadow and
  gradient CSS/defs while preserving default structural parity.
  A follow-up C4 text measurement slice fixed a baseline-environment seam: stored C4 upstream SVGs
  match `mmdc + chrome-headless-shell` text bboxes, so local C4 layout now uses a generated,
  C4-scoped headless-shell text lookup table keyed by font family, size, weight, and exact text
  before falling back to deterministic SVG bbox measurement. The key long C4 description now
  produces the expected `552px` box and C4 structural/root compare stays green.
  A follow-up all-supported raster audit slice then fixed the manual gate boundary for Treemap
  error-golden classDef bare-token fixtures and ran raster-enabled filtered audits across every
  implemented family, including full Flowchart. No new production visible-rendering defect was found
  in that scan. Continue HPD-080 only from a fresh failing gate, source-backed emitted-surface gap,
  or concrete consumer report; otherwise return to HPD-050 source-backed Architecture/Dagre/Graphlib
  audits before fine root residual work.
