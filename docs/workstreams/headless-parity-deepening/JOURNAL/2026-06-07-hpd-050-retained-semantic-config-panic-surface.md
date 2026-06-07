# HPD-050 - Retained Semantic Config Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

The shared config cleanup made `MermaidConfig` clone-on-write, merge, and drop paths stack-safe for
deep public config. Sequence and XYChart then removed typed-model compatibility bridges that
serialized through `serde_json::to_value(...)` or recursive `Value` clone paths.

Several remaining semantic JSON roots still retained `meta.effective_config` directly with
recursive `serde_json::Value::clone()`, and a few root objects wrapped that retained config through
`json!`. Host `site_config` is public API data, so a deep retained config should not make supported
semantic JSON parsing depend on Rust call-stack depth.

## Changes

- Replaced retained effective-config clones in Block, State, Treemap, Sankey, C4, and Architecture
  with `crate::config::clone_value_nonrecursive(...)`.
- Rebuilt C4, Sankey, and Architecture root JSON objects with `serde_json::Map` so the retained
  config is moved into the root object instead of being recursively wrapped by `json!`.
- Preserved Architecture's existing source-backed default `layout: "dagre"` insertion when the
  caller did not provide an explicit layout.
- Added `retained_semantic_config_handles_deep_public_config_with_small_stack`, which uses a
  `1,024`-level host config and validates Block, State, Treemap, Sankey, C4, and Architecture
  known-type semantic JSON parsing on a `128KB` stack.

## Verification

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core retained_semantic_config_handles_deep_public_config_with_small_stack` -
  passed, `1` test run.
- `cargo +1.95 nextest run -p merman-core block_render_model_uses_typed_variant_without_changing_json_parse treemap_render_model_uses_typed_variant_without_changing_json_parse parse_sankey_render_model_uses_typed_variant_without_changing_json_parse c4_render_model_uses_typed_variant_without_changing_json_parse` -
  passed, `4` tests run.
- `cargo +1.95 nextest run -p merman-core state architecture c4 block treemap sankey` - passed,
  `133` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.

## Boundary

The small-stack regression uses `parse_diagram_with_type_sync(...)` deliberately. An exploratory
auto-detect version overflowed before semantic parsing, inside the detector path, so that is a
separate detector-registry boundary and not evidence for or against retained config projection.

No parser behavior, SVG output, SVG baseline, root viewport formula, theme behavior, or
Architecture residual classification changed. This slice only removes recursive JSON clone/wrap
from retained semantic config projection.
