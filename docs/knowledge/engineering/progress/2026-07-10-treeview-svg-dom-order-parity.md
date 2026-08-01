---
type: Work Progress
title: Mermaid 11.16 TreeView SVG DOM order parity
timestamp: 2026-07-10T03:26:18+08:00
status: corrected
related_plan: docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md
git_branch: feat/mermaid-11-16-parity
git_commit: cca2ce09e562
tags: mermaid-11-16,treeview,svg-dom,ce-work
---

# Summary

TreeView SVG output now follows the Mermaid 11.16 baseline DOM shape instead of the earlier local
wrapper structure.

# Changed

- Removed per-node `<g>` wrappers from the TreeView parity renderer.
- Interleaved TreeView node text emission with horizontal connector lines so generated DOM order
  matches the upstream 11.16 fixtures under `<g class="tree-view">`.
- Preserved Mermaid 11.16's `treeView-node-dir` class and `font-weight: bold` rule for directory
  nodes. The earlier claim that this class was local-only was incorrect: the pinned
  `packages/mermaid/src/diagrams/treeView/renderer.ts` appends the class for directories and
  `styles.ts` defines the bold weight.
- Updated the TreeView SVG DOM and layout tests to assert the 11.16 class surface and to measure
  directory labels with the same bold font style that is emitted in SVG.

# Boundary

This is DOM structure alignment, not a layout heuristic change. The renderer still uses the
existing typed TreeView layout model. Directory emphasis is upstream rendering semantics, not a
local presentation choice; removing it would also make the headless label bbox inconsistent with
the emitted font style.

Mermaid issue #7954 is an upstream 11.16.0 regression for arrows between subgraphs. It should be
tracked as a pinned-baseline risk, not copied as a durable Merman semantic target.

# Next Action

Perform the final plan-level Definition of Done audit. The primary DOM parity comparison is green,
but broad root viewport residuals remain a secondary comparison concern and should not be accepted
with a blanket policy rule.
