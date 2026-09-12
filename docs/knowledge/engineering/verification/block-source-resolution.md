# Shared Block source resolution

The Block writer traversed nested nodes and merged later non-empty source fields,
while the theme source-fill mask read only top-level `blocks_flat` entries. A
nested node with an inline or class-owned fill could therefore be treated as
unowned during unsupported palette reconciliation. The new regression fails on
the previous implementation with an observed false mask instead of true.

`block/source.rs` now resolves effective source fields for geometry, ownership and
SVG emission using the same depth-first order and field replacement semantics.
It borrows strings and slices from the model. Empty fragments preserve earlier
fields; a non-empty style/class list replaces that list; `na` does not replace a
known shape; explicit column spans remain clamped to at least one. The writer
retains independent duplicate-terminal checking and deferred sink-error priority.

The regression covers nested inline and class-owned fills, empty later fragments,
and non-empty fragments that remove fill ownership. The palette evidence must be
NotApplicable only while source fill owns the visible node; removing that ownership
must leave a residual. Existing Block rendering, geometry, resource and receipt
checks protect the shared callers.

This is a prerequisite for NodeLabel migration, not its completion. Legacy route
counts and public support claims remain unchanged. No performance or fresh Web
artifact-size claim is made from removing field clones alone.

Verification passed: the focused Block/renderer suite ran 75 tests with 75 passed;
the complete private acceptance suite ran 138 tests with 138 passed; the full SVG
structure comparison and formatting checks passed. A clean-checkout run remains
to be recorded against the committed implementation.
