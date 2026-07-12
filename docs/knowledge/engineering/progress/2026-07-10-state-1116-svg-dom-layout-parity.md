---
type: Work Progress
title: State 11.16 SVG DOM and layout parity
timestamp: 2026-07-10T05:27:42+08:00
status: active
related_plan: docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md
git_branch: feat/mermaid-11-16-parity
tags: mermaid-11-16,state,svg-dom,layout,ce-work
---

# Summary

State diagrams were aligned to the Mermaid `@11.16.0` SVG DOM and layout surface after refreshing
the upstream State SVG baselines.

# Implemented

- Added State-scoped DOM ids, raw `data-id` retention, and `data-look` output for nodes, clusters,
  edges, labels, and click/link wrappers.
- Ported the 11.16 self-loop rendering shape: helper cyclic edges remain a layout implementation
  detail, but rendered self-loops use the original logical edge id and label id.
- Ported the 11.16 `fixCorners` edge post-processing before path generation and marker offsets.
- Aligned State node and note HTML label classes with upstream 11.16, including
  `markdown-node-label` and `noteLabel`.
- Aligned State styling/defs placement for classic, hand-drawn, and neo themes, including root
  defs and dependency marker CSS.
- Removed obsolete 11.12 rect-with-title span measurement overrides. State composite titles and
  descriptions now use the current HTML-like wrapping path.
- Regenerated State upstream SVG baselines and State layout goldens from the pinned 11.16 source.

# Important Boundaries

- Mermaid 11.16 composite self-loops should not reintroduce the rendered `cyclic-special-*` helper
  DOM. Those helpers are layout scaffolding, not the public SVG surface.
- Group/composite self-loops in the current 11.16 State baselines do not take the ordinary-node
  intersection/clipping path. Local rendering follows the source-backed 11.16 output rather than
  applying Flowchart-style cluster cuts.
- Known upstream Flowchart issue #7954 remains a separate pinned-upstream regression. It should not
  be used as justification for State-specific layout tuning or broad comparator normalization.

# Changed Areas

- Render: `crates/merman-render/src/svg/parity/state/{context,edge,node,render,style}.rs`.
- Layout: `crates/merman-render/src/state/layout.rs`.
- Removed overrides:
  `crates/merman-render/src/generated/state_text_overrides_11_12_2.rs`.
- Tests: `crates/merman-render/tests/state_svg_test.rs`.
- Fixtures: `fixtures/state/*.layout.golden.json` and `fixtures/upstream-svgs/state/*.svg`.
