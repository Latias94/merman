# HPD-050 - Remaining Retained Semantic Config Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

The first retained semantic config slice hardened Block, State, Treemap, Sankey, C4, and
Architecture. A follow-up scan still found recursive `meta.effective_config.as_value().clone()`
uses in the remaining public semantic JSON roots.

Host `site_config` is public API data. A deeply nested retained config should not make semantic
JSON parsing for supported diagram families depend on Rust call-stack depth, and moving a deep
`serde_json::Value` into a `json!` root object can recursively re-wrap it after a non-recursive
clone.

## Changes

- Replaced retained effective-config clones in GitGraph, Kanban, Packet, QuadrantChart, Radar,
  Requirement, and Mindmap with `crate::config::clone_value_nonrecursive(...)`.
- Rebuilt those public semantic root objects with `serde_json::Map` where retained config enters
  the root, preserving existing field names and optional/null JSON behavior.
- Covered both Mindmap semantic JSON paths:
  - normal roots with layout nodes, edges, shapes, and diagram id;
  - empty-root early return with empty `nodes` / `edges`.
- Preserved Mindmap's existing source-backed default `layout: "cose-bilkent"` insertion when the
  caller did not provide an explicit layout.
- Added
  `remaining_retained_semantic_config_handles_deep_public_config_with_small_stack`, which uses a
  `1,024`-level host config and validates GitGraph, Kanban, Packet, QuadrantChart, Radar,
  Requirement, Mindmap normal root, and Mindmap empty root through known-type semantic JSON parsing
  on a `128KB` stack.

## Verification

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core remaining_retained_semantic_config_handles_deep_public_config_with_small_stack` -
  passed, `1` test run.
- `cargo +1.95 nextest run -p merman-core parse_kanban_render_model_uses_typed_variant_without_changing_json_parse parse_packet_render_model_uses_typed_variant_without_changing_json_parse parse_requirement_render_model_uses_typed_variant_without_changing_json_parse parse_radar_render_model_uses_typed_variant_without_changing_json_parse parse_gitgraph_render_model_uses_typed_variant_without_changing_json_parse parse_quadrant_chart_render_model_uses_typed_variant_without_changing_json_parse mindmap_render_model_projects_same_look_and_theme_shape_as_json_model` -
  passed, `7` tests run.
- `cargo +1.95 nextest run -p merman-core gitGraph kanban packet quadrant radar requirement mindmap` -
  passed, `117` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.

## Boundary

This slice deliberately uses `parse_diagram_with_type_sync(...)` for small-stack evidence. It is
scoped to semantic JSON projection and does not claim detector-chain behavior.

No parser behavior, SVG output, SVG baseline, root viewport formula, theme behavior, or
Architecture residual classification changed. This slice only removes recursive JSON clone/wrap
from the remaining retained semantic config projection paths.
