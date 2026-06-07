# HPD-050 - Architecture Deep-Group Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

After Class namespace and dugong traversal were hardened, the remaining tree-shaped public input
candidate was Architecture. Mermaid `architecture-beta` syntax can create a chain of nested groups
with `group child(...) in parent`, and the local path routes that shape through manatee/FCoSE
compound layout before SVG group rectangles are computed.

## Red Signal

A deep public Architecture group chain split by phase:

- parse-only stayed green;
- layout overflowed on a small thread stack before the manatee/FCoSE traversal cleanup;
- after the manatee cleanup, deeper public layout chains became too slow for routine gates, so the
  public regression stays at `64` levels and deeper stack coverage moves to cheap unit tests.

## Changes

- Replaced manatee/FCoSE compound inclusion-depth recursion with explicit parent-chain traversal
  and memo backfill.
- Replaced manatee/FCoSE layout-base graph preorder recursion with an explicit owner-graph stack,
  preserving the same child ordering.
- Replaced Architecture SVG `GroupRectComputer::compute(...)` recursion with explicit enter/exit
  frames while preserving service/junction bounds, child-group inset behavior, debug output, group
  padding, and empty-group fallback sizing.
- Added regressions for:
  - public Architecture parse, layout, and SVG output through a `64`-level group chain;
  - manatee/FCoSE compound depth and layout order over a `2,048`-level compound chain on a `64KB`
    stack;
  - Architecture SVG group-rect computation over a `2,048`-level child-group chain on a `64KB`
    stack.

## Verification

- `cargo fmt --check -p manatee -p merman-render` - passed.
- `cargo nextest run -p manatee` - passed, `16` tests run.
- `cargo nextest run -p merman-render --test architecture_layout_test --test architecture_svg_test` -
  passed, `17` tests run and `1` skipped.
- `cargo nextest run -p merman-render group_rect_computer_handles_deep_child_group_chain_with_small_stack` -
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed.
- `git diff --check` - passed.

## Boundary

No SVG baseline, root override, Architecture root-bounds formula, or Mermaid parity fixture changed.
This is stack-safety hardening for a public Architecture group path plus its manatee/FCoSE and SVG
group-rect dependencies. It does not claim closure of Architecture `parity-root` diagnostics.
