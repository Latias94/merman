---
type: Work Progress
title: Upstream main integration during the U5 capability refactor
description: Preserve upstream theme and font repairs in presentation owners and verify the merged source.
timestamp: 2026-10-21T14:48:53Z
record_id: 8273ec7cff80431c84552aa12899bdae
producer_id: codex-root
run_id: semantic-capability-boundaries-u5
related_plan: docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md
git_branch: refactor/semantic-capability-boundaries
git_commit: 1a3a4d2f79d33072011d53383b8d09a2c5b1dac2
merge_head: 3119c614bf8c43d49cfe17ded0a53ec2e7fa983c
verified_tree: 00200fc33ea4f105bc55825c0211ed697432d590
---

# Integration

The maintainer requested integration of remote main. Saved the existing U5 work in
`1a3a4d2f7`, then merged the 40 remote-only commits through `3119c614b`. The tree
above contains the tested merge resolutions before this receipt. The merge was
uncommitted at capture time. A second fetch confirmed the same remote head.

- Keep source-selected palette authority, detector history, and paint ownership in
  `PresentationConfig`. Port the new paint predicate and private renderer bridge to
  those owners rather than restoring presentation state to lean `MermaidConfig`.
- Preserve upstream shared font stacks, theme program/outcome ownership, Gantt
  label states, C4 markers, ER color fallback, and ZenUML paint/measurement repairs.
  Adapt new full-config call sites and the new Gantt task fixture to canonical owners.
- Move upstream scalar-skipping iterative disposal and its small-stack regression
  to the generic `json_value` module. Do not restore a compatibility-only JSON owner.
- Resolve all conflicts without replacing entire files with one branch. An independent
  read-only reviewer compared the presentation ownership paths and 35 selected
  renderer paths against remote main and reported no concrete missing behavior.

# Verification

All Cargo builds ran serially in the existing F-drive checkout and target, with two
build jobs and two nextest threads. No additional worktree or target directory was
created. The index was empty before the checkpoint; all merge-stage paths were
checked against remote changes and the four additional refactor owner paths.

| Scope | Result | Nextest run ID |
| --- | --- | --- |
| Full core, all diagrams/presentation/compat/editor | 2,114 passed | `b5a1b6b2-438c-4930-a776-0d7fa57c129c` |
| Renderer library and 16 affected SVG integration targets | 3,874 passed, 3 skipped | `40d92840-b409-4869-b4de-7818db77f565` |
| Binding host theme contract and native fonts | 21 passed | `0682680d-91fa-4dae-ad98-c6040fd554d8` |
| Facade Flowchart effect PNG export | 5 passed | `510c6802-b381-4dad-9e56-9c8906a0943b` |
| Export library and native font integration | 104 passed | `badbbe40-7be4-4c70-8c1c-2d4a2939d500` |
| Canonical union of the 31 migrated selectors | 453 passed | `d8fda188-2129-4d3f-8f68-60bde2ae287a` |

Production full-core `cargo clippy --lib -- -D warnings`, `cargo fmt --all --check`,
and the staged whitespace check passed. Initial compilation identified two moved
core helper/test references and one newly added Gantt structure literal; corrected
those ownership adaptations before the successful runs. No test assertion was
weakened to make the merge pass. This Windows receipt does not claim Linux
fontconfig validation or a complete workspace/feature matrix.

# Plan Impact

The approved R1-R16 requirements and U1-U9 sequence remain valid. No change to the
Mermaid pin, stable transport ABI/schema, or explicit math policy was identified.
U6 must preserve the new shared font/program ownership. U8 must account for the
explicit native-font `ttf-parser` dependency and current generated/legal contracts.
U9 must identify the merged source, lockfile, and artifact inputs: upstream font,
theme, and JSON disposal optimizations are not semantic-refactor improvements.

This is a main-integration checkpoint, not completion of U5 or the active goal.
Class, GitGraph, and Agentflow canonical proposals remain unapplied; actual editor
feature isolation, consumer migration, final package checks, comparable artifact
measurements, and the final review remain. No push, PR, release, or publication
was performed.
