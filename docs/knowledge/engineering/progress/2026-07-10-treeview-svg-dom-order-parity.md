---
type: Work Progress
title: Mermaid 11.16 TreeView SVG DOM order parity
timestamp: 2026-07-10T03:26:18+08:00
status: active
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
- Removed the local-only `treeView-node-dir` class and CSS rule. Mermaid 11.16 preserves user CSS
  classes such as `highlight`, but does not add this directory class in the baseline SVGs.
- Updated the TreeView SVG DOM test to assert the 11.16 class surface.

# Boundary

This is DOM structure alignment, not a layout heuristic change. The renderer still uses the
existing typed TreeView layout model and only changes serialization order and local-only classes.

Mermaid issue #7954 is an upstream 11.16.0 regression for arrows between subgraphs. It should be
tracked as a pinned-baseline risk, not copied as a durable Merman semantic target.

# Next Action

Perform the final plan-level Definition of Done audit. The primary DOM parity comparison is green,
but broad root viewport residuals remain a secondary comparison concern and should not be accepted
with a blanket policy rule.
