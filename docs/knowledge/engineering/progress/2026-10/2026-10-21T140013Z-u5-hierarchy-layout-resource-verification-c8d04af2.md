---
type: Work Progress
title: U5 hierarchy and layout families with resource boundary verification
description: Canonical parsing reaches 31 selectors; explicit serialization preserves full resource accounting.
timestamp: 2026-10-21T14:00:13Z
record_id: c8d04af271aa4a139b5852ab8e20cd55
producer_id: codex-root
run_id: semantic-capability-boundaries-u5
related_plan: docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md
git_branch: refactor/semantic-capability-boundaries
git_commit: 55c3b0489
---

# Integrated Work

This records uncommitted progress after `55c3b0489`, not completion of U5 or the goal.
The existing F-drive checkout and target remain the only implementation/build workspace.
Cargo execution is serial, with two build jobs and two nextest threads.

- Integrated canonical ER, Architecture, Gantt, and C4. C4 owns typed records instead
  of a JSON-map master. Its deferred relation-type validation preserves redeclaration
  repair and the existing final error. Gantt retains semantic URL/callback security;
  display mode and computed task end times belong to presentation.
- Integrated Ishikawa, Treemap, Block, Kanban, XYChart, and Mindmap proposals. Root
  owns central catalog/enum/callback/resource and renderer integration. Canonical
  models retain authored hierarchy, styles, metadata, Markdown, and decorations.
  Requested presentation owns dimensions, palette indices, and computed overrides.
- Exposed the generic iterative `ManagedJson` owner independently of `compat-json`.
- Preserved ordinary canonical/full/JSON omission of editor observation. A read-only
  review found that XYChart still copied entire successful list/range payloads into
  editor-only string carriers. Those carriers now contain `SourceSpan` only; semantic
  strings, grammar validation, and error spans remain intact.
- Fixed logical Block depth accounting after flattening. A 32-level parsed hierarchy
  was measured as depth 3 before the fix; the regression now covers depths 32 and 130.
- Fixed Gantt static-field resource inflation from `serde(flatten)`: the empty full
  model was counted as 18 entities instead of 1. Explicit `SerializeStruct` preserves
  previous full field names, order, and counting for the diagram and task wrappers.
- XYChart and Radar wrappers serialize transparently through their only persisted
  canonical field. This avoids a static-map-key accounting change. XYChart's observed
  regression was items/text 18/69 instead of 11/22 for the same chart.
- Kanban canonical metadata uses iterative item/text/depth counting. Treemap now
  counts arbitrary value depth as well as hierarchy depth; the latter fixes an
  inherited manually-constructed-model gap. One JSON traversal replaces two.
- Two existing renderer tests assumed unavailable diagram parsers. Fixture selection
  now follows diagram features, preserving full-combination assertions.

# Verification

| Scope | Result | Nextest run ID |
| --- | --- | --- |
| DB family canonical union | 244 passed | `6857e2b6-a629-4e8b-9030-8103bb5a9a2c` |
| DB full family tests | 166 passed | `fbab0c89-490d-4c68-9207-c34e317be259` |
| C4 canonical selection | 233 passed | `0286ced9-ced8-4f6f-a6a8-32fb1626d646` |
| C4 full family tests | 77 passed | `285dbd33-d2ad-4ff2-a501-c8a1af82d359` |
| Block logical depth before correction | Failed, 3 versus 33 | `562b76b0-83a5-44d3-a021-ceb6b4f51614` |
| Hierarchy canonical union after correction | 240 passed | `4bfb2212-7b75-4ed2-bf24-93db1f68d23b` |
| Hierarchy full family/resource tests | 109 passed | `6a266128-0caa-4508-8eb2-2d36c6aa2c12` |
| Selected hierarchy/Gantt/Radar renderer library | 1,379 passed, 1 skipped | `f005752d-7532-414c-a1e5-62f424a1e9c0` |
| Gantt full-resource regression before correction | Failed, 18 versus 1 | `321638fa-4015-4eb0-b749-48a2953a4877` |
| Gantt family/resource tests after correction | 92 passed | `fa6e7aed-522c-426f-bfcb-33a7b4dc4613` |
| XYChart/Kanban/Treemap resource regressions before correction | 3 expected failures | `da23ab2c-de5a-434e-aa56-0f4dec62d845` |
| Full core, all diagrams/presentation/compat/editor | 2,111 passed | `1b1f43dd-b328-42ec-86df-d4a209de47ce` |
| XYChart after span-carrier change plus resource regressions | 40 passed | `49bed74d-5ad2-4596-aed7-43d26bc2ddc9` |
| 31 migrated selectors, no presentation/compat/editor | 452 passed | `b26c2aea-f1a1-421b-b32d-6cd93d4a544f` |
| Same 31 selectors with presentation, no compat/editor | 641 passed | `1b2fc61d-619a-4d57-9673-1df61bd5d3bf` |

Commands use `cargo nextest run -p merman-core --no-default-features --lib
--build-jobs 2 --test-threads 2`, with the corresponding selectors. The 31-selector
union is the manifest's `all-diagrams` list excluding Agentflow, Class, and GitGraph.
The renderer command selects Ishikawa/Treemap/Block/Gantt/Radar with defaults off.
Tests cover exact and one-below metadata/value nesting ceilings at depths 32 and 130.
The full core result precedes the XYChart span-only change; the focused 40-test and
both union results follow it. Clippy/final formatting were pending at capture time.

# Independent Review

Read-only DB/C4, hierarchy, and layout reviewers ran no Cargo and wrote no source.
Root reproduced and fixed the Gantt resource and XYChart discarded-string findings.
No additional confirmed output/security/error regressions were reported.
Treemap standalone node serde no longer carries `cssCompiledStyles`; migration must
direct full callers to `visuals.root_styles` / `visuals.node_styles`. The previously
reported arbitrary-value nesting residual is now covered by the passing regression.

# Next Actions

- Complete current verification/checkpoint, then merge the user-requested remote main.
  Fetch observed `origin/main=3119c614b`, merge base `482fcde7f`, with 42 local and
  40 remote commits. Six uncommitted paths overlap remote changes. Preserve upstream
  JSON disposal optimization in the new generic owner and palette ownership in the
  presentation configuration, rather than restoring obsolete module ownership.
- Integrate the ready Class artifact while preserving full typed serialization.
  Agentflow and GitGraph native workers are preparing family-local artifacts only.
- GitGraph's warmup comment conflicts with the current render-only fixture generator
  and pinned Mermaid parse lifecycle. Verify the current oracle before changing seeded
  ID expectations; do not create a second evaluator to preserve an obsolete workaround.
- Actual editor compile isolation, Flowchart authored CSS ownership, real lean
  consumers/prepared adapters, legal/package contracts, and final isolated stripped
  artifact/performance evidence remain required under U5/U6/U8/U9.
- Public feature naming was assessed, not renamed. Do not claim those boundaries are
  finished merely because their manifest entries exist.

No push, PR, publication, release, new worktree, or C-drive build was performed.
