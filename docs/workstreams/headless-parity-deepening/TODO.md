# Headless Parity Deepening - TODO

Status: Active
Last updated: 2026-06-02

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
  edge-object parameters to `EdgeKey`, without adding JS argument overloading. ARCH-022's first
  Dagre reference adapter slice is also landed:
  `dagre_reference.rs` now owns the Rust-side
  reference input/output schema, JS harness invocation, compound-edge normalization, and Rust/JS
  delta extraction, while `compare-dagre-layout` remains a State-only graph producer. Basic,
  composite, and internal-cluster State comparisons all stayed zero-delta. Architecture Cytoscape
  service-label measurement now has a shared `ArchitectureCytoscapeServiceLabelExtension` seam used
  by both FCoSE node `BoundsExtras` and SVG root/group service-bounds estimation. That seam kept SVG
  root `createText(...)` measurement separate from Cytoscape compound-child label measurement and
  preserved the then-known 26 Architecture root residuals. A disconnected-islands root-bounds audit
  then
  rejected a tempting global top-level-service switch from `svg_root_bounds` to
  `cytoscape_group_child_bounds`: it made that one height-only row exact but expanded full
  Architecture root mismatches from `26` to `84`. The first narrow phase-specific follow-up is now
  landed: isolated top-level services in diagrams that also have groups use the Cytoscape
  group-child phase for root contribution, making the disconnected-islands row exact and reducing
  Architecture root mismatches from `26` to `25` while preserving structural parity.

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
  style elements, and, under the `raster` feature, SVGs that cannot convert to PNG.
  Continue by scanning supported diagrams for blank output, hidden labels, black blocks, lost theme
  colors, and other functional renderability failures before returning to fine root residual work.
