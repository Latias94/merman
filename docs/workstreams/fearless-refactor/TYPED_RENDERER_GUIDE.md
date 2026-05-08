# Adding a Typed Diagram Renderer

This guide records the preferred path for moving a diagram away from JSON fallback rendering.

## Goal

The render path should consume a typed `RenderSemanticModel` without rebuilding the full semantic
JSON payload. The stable `parse_diagram_sync` semantic JSON API should remain compatible unless a
public API change is explicitly planned.

## Steps

1. Add or reuse the diagram's typed render model in `merman-core`.
2. Add a render parse helper such as `parse_<diagram>_model_for_render`.
3. Add a `RenderSemanticModel` variant and route `parse_diagram_for_render_model_sync` through it.
4. Add a typed layout entrypoint in `merman-render` and route
   `layout_parsed_render_layout_only` through it.
5. Add a typed SVG model entrypoint and route
   `render_layout_svg_parts_for_render_model_with_config` through it.
6. Keep the semantic JSON compatibility path honest: either deserialize into the shared typed model
   or leave the old JSON path only when the public compatibility contract requires it.
7. Update `RENDER_MODEL_INVENTORY.md`, `TODO.md`, and a performance spotcheck note when the
   migration changes hot render behavior.

## Gates

Use `GATES.md` as the source of truth. For a focused typed migration, the minimum useful set is:

```sh
cargo fmt
cargo clippy -p merman-core -p merman-render --all-targets --all-features -- -D warnings
cargo nextest run -p merman-core -p merman-render
cargo run -p xtask -- compare-<diagram>-svgs --check-dom --dom-decimals 3
```

Before a release-boundary commit, run:

```sh
cargo run -p xtask -- verify --strict
```

## Review Checklist

- The typed render path avoids owned semantic JSON construction.
- The compatibility JSON path still returns the same shape for `parse_diagram_sync`.
- Layout and SVG dispatch have one typed owner each.
- New override entries are not used to hide model or layout bugs.
- Benchmark or timing evidence records the cost impact when the path is performance-sensitive.
