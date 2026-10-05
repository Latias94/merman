---
type: "Work Progress"
title: "Mermaid 12 remaining ELK kernels"
description: "Mr Tree Radial Rectpacking and SPOrE kernels with real elkjs evidence and EPL notices; dispatch remains next."
timestamp: 2026-09-20T08:32:43Z
record_id: "6ca99d794c984fc4911a7c54aad95637"
producer_id: "codex-mermaid12"
run_id: "session-01a0bd10"
related_plan: "docs/plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md"
git_branch: "refactor/mermaid-12-alignment"
git_commit: "bd706ac86"
supersedes: "82a86582d1cf41399496f41dd8ab4acd"
---

# Summary

The complete U1-U15 / R1-R12 Mermaid 12 goal remains active and far from finished. This turn
made concrete progress by implementing the remaining additional ELK kernels. Commit `bd706ac86`
contains Mr. Tree, Radial, Rectangle Packing, and SPOrE overlap removal, their actual elkjs
oracle tests, README scope and generated EPL notice updates. No renderer dispatch or production
bundle/default promotion happened. No push, PR or release. The prior goal turn was progress.

# Implemented kernels

All translations remain inside `crates/merman-elk-layered/src/algorithms/`, EPL-2.0, pinned ELK
0.9.1 commit `62d5909f96fad541bc101ad52dabaece6b7eab7e` and elkjs 0.9.3. Node measurement,
source micro layout, hierarchy scheduling and effective option projection remain adapter work.

- `mrtree/`: worker elk_mrtree implemented source DFS treeification, Walker placement/apportion,
  all four directions, AvoidOverlap routing with shared long-edge gaps/cycle side channels,
  component processing and packing. Explicit stacks avoid recursive walk overflow. Source
  details include component-edge duplication, label-based edge identity, synthetic root id,
  normalization using first child padding and unusual final extent calculation. Unsupported
  BFS, FAN/CONSTRAINT and optional compaction are not advertised. Tests include 16 actual
  elkjs numerical cases and malformed/work/numeric cases; temporary oracles are `target/mrtree*-oracle.*`.
- `radial/`: worker elk_radial implemented default Eades placement, NODE_SIZE wedges,
  radius-extension overlap removal, graph sizing and straight-line endpoint clipping. First
  root, disconnected nodes, shared successors, source-port edges and strict source overlap
  predicates are preserved. Reachable cycles fail instead of recursing indefinitely. Eight
  actual elkjs examples plus cancellation, invalid geometry, and a 2048-node depth case.
  Oracles: `target/mermaid-12-admission/radial/`. Radial is a container algorithm only; do not
  register a root `elk.radial` loader.
- `rectpacking/`: worker elk_rectpacking implemented greedy width estimation, block/stack
  compaction, repeated width adjustment, equal whitespace expansion, content translations,
  minimum dimensions and source trybox branch. True stackable examples execute compaction;
  Box is used only when RectPackingLayoutProvider itself selects it. The source's original
  ROWS/additional-height retention across repeated compaction is preserved. Nine numerical
  examples plus mid-compaction cancellation; oracle script `target/rectpacking-oracle.cjs`.
  Important: Mermaid emits `SCANLINE`, but ELK 0.9.1 only knows GREEDY/TARGET_WIDTH. Actual
  elkjs falls back to GREEDY. Provider defaults do not expand whitespace; Mermaid's preset does.
- `spore_overlap/`: root implemented source scanline overlap detection, Bowyer-Watson
  triangulation, NaiveMinST and tree growth, source importer margins/root selection,
  normalization and straight-edge clipping. The target is clipped toward the already clipped
  source endpoint. Caller owns the random stream used only for duplicate centers; precision
  stalls and invalid random samples are typed errors. GWT Double hash truncation, reversed
  y bits and insertion-ordered hash buckets preserve equal-cost edge tie behavior. Existing
  source scanline early-break and fuzzy-comparison semantics are retained. Six actual elkjs
  geometries plus three option cases, deterministic duplicate jitter and budget/numeric tests.
  Oracles: `target/mermaid-12-admission/spore-{oracles,options}.cjs`, `spore-oracles.json`.

The public kernel APIs are deliberately bounded to Mermaid-reachable provider settings. They
must not be relabeled as root/container Mermaid support before dispatch and hierarchy work.
The new kernels received root source/logic inspection and worker self-review; the goal's final
independent review is still pending, including these modules and their eventual adapters.

# Evidence

Final changed-kernel run:

`cargo nextest run -p merman-elk-layered -E 'test(algorithms::spore_overlap) | test(algorithms::radial) | test(algorithms::mrtree) | test(algorithms::rectpacking)'`

18 passed, no failures, run `93917add-67d2-4907-ba98-4a865da7f2df`. Earlier scope executions
passed Mr. Tree/Radial and Rectpacking on their first runs. The SPOrE nondefault-options test
initially omitted the oracle input's parent center (150,100); providing the same input fixed
that test without changing the algorithm or expected output. Focused corrected run
`3d94a91d-0d67-4ce3-88db-0e9a6ba56791` passed, followed by the combined final run.

`cargo fmt --all -- --check`, `git diff --check`,
`python scripts/verify-third-party-licenses.py`, and
`python scripts/sync-release-legal-materials.py --check` passed (382 distribution projections).
The canonical notice now names all eight translated algorithm families and existing generators
produced the affected copies. No dependency/feature changes or new verification scripts.
Cargo ran serially with the normal target; workers did not run Cargo. All workers completed.

# Next action and full remaining scope

Next implement source-backed typed root/container dispatch and bottom-up compound handling in
`merman-layout-elk`, with measured family projections and real geometry/label/port ownership.
Keep the two target allowlists distinct: root `elk` is layered; additional roots are
`elk.stress`, `elk.force`, `elk.mrtree`, `elk.sporeOverlap`, `elk.box`, `elk.rectpacking`.
Container selection also permits radial. Do not register root `elk.layered` or `elk.radial`.
Use existing operation work/seed admission; no fallback after execution/cancellation errors.
Complete node micro layout and source minimum-size/content translation before advertising
mixed container algorithms. Packing providers do not route edges; follow target cleanup.

The previous checkpoint's remaining U5 postprocessing, State/Requirement/Mindmap paths,
Agentflow/Usecase, appearance/default promotion, other family changes, editor/binding/features,
reference admission, final size/performance and independent review all remain. The production
bundle still selects 11.17.2; target runtime/config/theme artifacts remain staged and must not
be promoted as-is. No previous fixture provenance was relabeled. Source identities and trusted
base are unchanged from predecessor records. Use this shard for continuity; legacy root
memory rollups remain untouched and do not need migration for this task.
