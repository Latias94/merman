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
  non-multigraph panics, and named queries/removals no longer alias the unnamed edge. ARCH-022's
  first Dagre reference adapter slice is also landed: `dagre_reference.rs` now owns the Rust-side
  reference input/output schema, JS harness invocation, compound-edge normalization, and Rust/JS
  delta extraction, while `compare-dagre-layout` remains a State-only graph producer. Basic,
  composite, and internal-cluster State comparisons all stayed zero-delta.

## M5 - Semantic / Render Unification Pilot

- [ ] HPD-060 [owner=codex] [deps=HPD-020,HPD-040] [scope=crates/merman-core,crates/merman-render]
  Goal: Pick one high-value diagram family and deepen the “one semantic truth, adapters on top”
  direction to reduce typed-vs-JSON parallel master paths.
  Validation: pilot design note + focused implementation slice or an explicit split decision with
  narrowed scope.
  Review: This is a seam-deepening pilot, not a full repo-wide migration in one task.
  Evidence: docs/workstreams/headless-parity-deepening/EVIDENCE_AND_GATES.md
  Context: docs/workstreams/headless-parity-deepening/CONTEXT.jsonl
  Handoff: Sequence and Architecture are the default pilot candidates unless evidence changes.

## M6 - New Family Rubric

- [ ] HPD-070 [owner=planner] [deps=HPD-010,HPD-030] [scope=docs/alignment,repo-ref/mermaid/packages/mermaid/src/diagrams]
  Goal: Define a durable rubric and priority order for unsupported Mermaid families so new scope is
  promoted intentionally and only when it fits the headless architecture.
  Validation: rubric and priority list documented; `treeView`, `venn`, `ishikawa`, `eventmodeling`,
  `wardley`, `railroad-*`, and `cynefin` are classified.
  Review: Do not silently imply these families are “next up” without a capability fit argument.
  Evidence: docs/workstreams/headless-parity-deepening/EVIDENCE_AND_GATES.md
  Context: docs/workstreams/headless-parity-deepening/CONTEXT.jsonl
  Handoff: This task should make later family workstreams easier to open, not start them.
