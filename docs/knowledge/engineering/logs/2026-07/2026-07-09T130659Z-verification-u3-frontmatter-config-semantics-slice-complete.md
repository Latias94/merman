---
type: "Work Log"
title: "U3 frontmatter/config semantics slice complete"
description: "Shared parsing now follows Mermaid 11.16 frontmatter indentation semantics and 11.16 config namespace compatibility."
timestamp: 2026-07-09T13:06:59Z
producer_id: "codex-root"
related_plan: "docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md"
git_branch: "feat/mermaid-11-16-parity"
---

# Summary

U3 aligned shared frontmatter and config plumbing with Mermaid 11.16. Detector-side frontmatter
stripping now reuses the same parsed block semantics as preprocessing, so a closing `---` only
closes a frontmatter block when it uses the same horizontal indentation as the opening delimiter.
Malformed mismatched-indentation frontmatter remains visible and flows to the `---` pseudo-diagram
error path instead of being silently stripped.

The local frontmatter top-level diagram namespace compatibility layer now includes 11.16 namespaces
that are present in generated defaults and local capability facts, including `swimlane`, `cynefin`,
`railroad`, `treeView`, `eventmodeling`, and `treemap`.

# Verification

- `cargo fmt --check`
- `cargo nextest run -p merman-core detector_registry_requires_matching_frontmatter_indentation parse_rejects_mismatched_indented_frontmatter_like_upstream parse_maps_top_level_frontmatter_diagram_config parse_indented_frontmatter_like_upstream --no-fail-fast` passed: 4/4.
- `cargo nextest run -p merman-core frontmatter config directive detect --no-fail-fast` passed:
  101/101.
- `cargo run -p xtask -- check-alignment` passed.

# Next

U4 should focus on source-backed semantic deltas in existing families before broad SVG baseline
refresh: ER attributes, State multi-word state errors, XYChart labels/rotation, Pie 11.16 config,
TreeView box-drawing/icons, and Architecture seeded layout behavior.
