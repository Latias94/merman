# HPD-050 - COSE-Bilkent Radial Tree Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

After the shared config/directive/frontmatter hardening slice, the next audit pass looked for
remaining production recursion and panic candidates that were reachable from public parser/render
paths. `manatee`'s COSE-Bilkent implementation still positioned flat forest branches through
recursive `branch_radial_layout(...)`; Mindmap layout reaches this through public
`layout_indexed(...)`.

The existing Mindmap deep-chain regression used the default thread stack, so it proved broad
functionality but did not prove the layout algorithm was independent of Rust call-stack depth.

## Changes

- Replaced recursive COSE-Bilkent radial branch descent with explicit heap-backed `BranchFrame`
  traversal.
- Preserved the previous semantics:
  - node angle calculation;
  - parent-edge skip;
  - child traversal order;
  - per-child start/end angle assignment;
  - radial distance increment.
- Added a `64KB` stack regression through public `layout_indexed(...)` for a `2,048`-node tree.

## Verification

- `cargo +1.95 nextest run -p manatee layout_indexed_handles_deep_tree_radial_layout_with_small_stack` -
  passed, `1` test run.
- `cargo +1.95 nextest run -p manatee` - passed, `17` tests run.
- `cargo +1.95 nextest run -p merman-render --test mindmap_svg_test` - passed, `4` tests run.
- `cargo +1.95 fmt` - passed.

## Boundary

No COSE-Bilkent force constants, Mindmap SVG baselines, root viewport formulas, Architecture
residuals, or renderer theme/style behavior changed. This is shared layout stack-safety hardening.
