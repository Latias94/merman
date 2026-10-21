---
type: Work Progress
title: U5 canonical family expansion and JSON lifecycle ownership
description: Eleven more families compile as canonical semantics, with full core behavior revalidated.
timestamp: 2026-10-21T11:25:04Z
record_id: 5ba5a6b4ab4b47dba12c820946a04370
producer_id: codex-root
run_id: semantic-capability-boundaries-u5
related_plan: docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md
git_branch: refactor/semantic-capability-boundaries
git_commit: 55c3b0489
---

# Integrated Work

This is uncommitted progress beyond `55c3b0489`, not a U5 or goal completion claim.
All work uses the existing F-drive checkout and target. Cargo runs remain serial with two
build jobs and two nextest threads. No push, PR, release, or publication occurred.

- Moved the generic iterative JSON owner from `compatibility_json.rs` to
  `json_value/managed.rs` as `ManagedJson`. The public `ManagedSemanticJson` spelling
  remains gated by `compat-json`; generic deep ownership no longer requires presentation.
- Added canonical TreeView model/entry, removed redundant semantic-source wrappers,
  and omitted editor observation at actual collection sites during ordinary parsing.
- Integrated native patch proposals for Railroad/ZenUML, QuadrantChart/Radar/Sankey/Venn,
  and Requirement/Usecase/EventModeling/Wardley. Root alone applied source changes and
  wired the existing catalog, semantic enum, and existing resource counters.
- Radar full output now owns one canonical model plus its private compatibility sentinel.
  Its specialized resource counter borrows the canonical model; it does not clone it.
- Fixed an integration-only misplaced Wardley derive and a resource test that needed
  a full Radar wrapper. Removed repeated nested Wardley observation demand conditions.
- Replaced brittle exhaustive negative test arms with test-local wildcard arms so
  adding a canonical family does not require duplicating feature lists in assertions.

# Verification

Authoritative nextest results:

| Scope | Result | Run ID |
| --- | --- | --- |
| No default features, `test(json_value)` | 4 passed | `5e2d7d52-9d70-435a-a67d-146d4c2d88d0` |
| Pre-change full TreeView characterization | 24 passed | `ff8e81a4-0f32-429a-be22-5f1566437a2e` |
| TreeView-only semantic library tests | 233 passed | `f5ddf653-d325-45a7-9b50-3e7e609e27b3` |
| Eleven newly migrated family semantic union | 312 passed | `1a7b390b-34af-4761-aaec-c2f8015f3e57` |
| All diagrams + presentation + compat-json + editor-facts | 2,062 passed | `c820946a-b8d7-4fa5-8c37-0c63ac043700` |

Commands use `cargo nextest run -p merman-core --no-default-features --lib
--build-jobs 2 --test-threads 2`; scope-specific selectors follow `--features`.
The eleven-family set is `diagram-tree-view,diagram-railroad,diagram-zenuml,
diagram-quadrant-chart,diagram-radar,diagram-sankey,diagram-venn,diagram-requirement,
diagram-usecase,diagram-event-modeling,diagram-wardley`.
The first no-family check covers 2,048-level JSON clone/equality/export/drop on a
128 KiB stack and cancellation before writer output.

Worker evidence was limited to read-only source inspection, in-memory patch replay,
and rustfmt via stdin/stdout. Workers did not run Cargo or claim red/green evidence.
Their artifacts are ignored scratch under `target/agent-patches/`.

# Remaining Work

- Full `all-diagrams` without presentation remains unsupported while thirteen families
  still require migration: Class, ER, Architecture, Gantt, Kanban, XYChart, Mindmap,
  C4, GitGraph, Agentflow, Ishikawa, Treemap, Block. Do not gate them away to obtain green.
- Presentation without compat-json is being tested for the twenty-one migrated families;
  no result is claimed here.
- `editor-facts` still lacks a real compile-time boundary. Ordinary editor observation
  removal is not proof that editor definitions and recovery helpers are excluded.
- Flowchart authored subgraph/vertex CSS precedence must move to canonical ownership
  before ASCII can stop requiring presentation. Preserve declaration-order precedence.
- Real lean consumers, prepared/binding ownership, package/legal checks and final
  isolated stripped artifact/performance evidence remain required under U6/U8/U9.
- Feature naming was discussed, not renamed. `operation-deadlines` is currently a
  platform clock backend switch on WebAssembly, not a universal deadline switch.
  `compat-json` means Mermaid wire compatibility, not ordinary serde serialization.

# Active Proposal Work

Two native workers are preparing patches only, without source edits or Cargo:
`u5_db_family_patches` owns ER/Architecture/Gantt proposal files;
`u5_layout_family_patches` owns Kanban/XYChart/Mindmap proposal files.
`u5_family_integration_review` is independently reviewing this integrated batch.
The canonical source tree and actual verification outputs remain authoritative.
