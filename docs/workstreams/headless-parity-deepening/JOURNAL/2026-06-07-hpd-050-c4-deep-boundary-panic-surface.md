# HPD-050 - C4 Deep-Boundary Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

After Block's deep composite path was converted away from recursive traversal, the next
panic-surface audit targeted C4. C4 core semantic output is already flat, but render layout still
recursed over nested boundary/deployment-node hierarchy. A `1,500`-level C4 boundary chain
reproduced stack overflow through the public render-model layout path.

## Changes

- Replaced recursive `layout_inside_boundary(...)` calls with an explicit heap-backed boundary
  frame stack.
- Kept the existing parent-bounds accumulation semantics:
  - sibling boundary rows share the same per-level `current_bounds`;
  - shapes are laid out before child boundaries;
  - child boundaries expand the pending parent before that parent boundary is finalized;
  - root dimensions still come from accumulated C4 global bounds.
- Added a public-path regression that parses a `1,500`-level C4 boundary chain through
  `parse_diagram_for_render_model_sync(...)` and layouts it through
  `layout_parsed_render_layout_only(...)`.

## Verification

- `cargo nextest run -p merman-render --test c4_svg_test c4_public_layout_handles_deep_boundary_chain` -
  first run failed before the fix with stack overflow; passed after the explicit-stack layout
  traversal.
- `cargo nextest run -p merman-render c4` - passed, `6` tests run.
- `cargo fmt --check -p merman-render` - passed.
- `cargo run -p xtask -- compare-c4-svgs --check-dom --dom-mode parity --dom-decimals 3` - passed.
- `git diff --check` - passed.

## Notes

- No SVG baseline, root override, Architecture root-bounds formula, or Mermaid parity fixture was
  changed.
- This slice hardens C4's accepted deep boundary path. It does not introduce a new C4 depth limit.
