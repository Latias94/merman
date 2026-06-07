# HPD-050 - Architecture iconText XHTML Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

After Architecture deep group traversal was hardened, the next public Architecture input surface was
service `iconText`. Mermaid `architecture-beta` service syntax accepts quoted icon text, and local
rendering supports XHTML/SVG-like fragments inside a service icon foreignObject.

The fragment parser already used an explicit stack, but the parsed fragment tree then went through
namespace rewriting and serialization before raw XHTML normalization. Those two phases still used
recursive tree traversal over user-authored markup.

## Red Signal

A public `architecture-beta` service with a deeply nested `<span>...Icon...</span>` `iconText`
fragment reproduced stack overflow in the SVG render path before this slice.

During diagnosis, `sanitize_text(...)`, direct Architecture render-model parsing, and detector-only
checks were separated from the failing path. The current fix is scoped to the renderer's
foreignObject fragment handling, not to sanitizer behavior or Architecture layout formulas.

The user-reported `architecture_layout_handles_deep_group_chain` abort was also rechecked as a
focused single test on the current worktree and passed, so it was not treated as a regression from
this slice.

## Changes

- Replaced recursive `rewrite_foreign_object_fragment_nodes(...)` traversal with explicit
  heap-backed frames while preserving SVG/HTML namespace classification and the existing behavior
  that moves HTML children out of SVG-only parents after the first non-SVG child.
- Replaced recursive fragment serialization with a consuming explicit stack.
- Took child vectors before element frames are dropped, so deep fragment trees do not overflow
  during traversal or cleanup.
- Added regressions for:
  - public Architecture SVG output through a `1,200`-level nested XHTML `iconText` fragment;
  - lower-level foreignObject normalization through a `2,048`-level nested XHTML fragment on a
    `64KB` stack.

## Verification

- `cargo nextest run -p merman-render architecture_svg_handles_deep_icon_text_xhtml_fragment` -
  passed.
- `cargo nextest run -p merman-render normalize_xhtml_fragment_handles_deep_nested_html_with_small_stack` -
  passed.
- `cargo nextest run -p merman-render architecture_layout_handles_deep_group_chain` - passed.
- `cargo nextest run -p merman-render --test architecture_layout_test --test architecture_svg_test` -
  passed, `18` tests run and `1` skipped.
- `cargo fmt --check` - passed.
- `git diff --check` - passed.

## Boundary

No SVG baseline, root override, sanitizer behavior, Architecture root-bounds formula, or Mermaid
parity fixture changed. This is stack-safety hardening for Architecture foreignObject XHTML
normalization and does not claim closure of Architecture `parity-root` diagnostics.
