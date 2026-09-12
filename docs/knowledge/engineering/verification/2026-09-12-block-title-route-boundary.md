# Block title theme route boundary — 2026-09-12

## Current state

Block's typed theme plans currently own base `FontStack`/`FontSize` and `Node.fill`/`Node.stroke`.
The Block SVG writer still reads `title_color` from the Mermaid node-diagram theme adapter and
emits it in the shared CSS rule for `.flowchartTitleText` and cluster labels.

The Block render model and family artifact do not currently carry a diagram-title occurrence or a
writer-owned title terminal receipt. The CSS selector therefore does not prove that a title exists,
was visible, or consumed a typed `ThemeTarget::Title` winner.

## Migration boundary

Do not classify Block `Title.fill` as `TypedAdapter` or remove its compatibility projection until
all of the following exist:

1. Block preparation identifies the actual title occurrence and its visibility.
2. A writer-owned plan resolves the typed title fill with source/config ownership precedence.
3. Layout, CSS emission, and terminal evidence use the same resolved winner.
4. Title-less, hidden, source-owned, and explicit site-config cases are recorded as
   `NotApplicable` rather than `Applied`.
5. Native SVG and PNG witnesses prove the visible title fill and strict portability accepts the
   typed route.
6. The cutover manifest and tombstone remove only the proven title projection.

A CSS rule that merely contains `.flowchartTitleText` is insufficient evidence because Block may
emit no matching title element. The first implementation should add the semantic occurrence and
receipt before changing the family mechanism matrix.

## Related evidence

- `crates/merman-render/src/svg/parity/block/render.rs` emits the shared `.flowchartTitleText`
  rule from the Mermaid adapter's `title_color`.
- `crates/merman-render/src/block/theme.rs` currently has node-paint and typography plans, but no
  title paint plan.
- `crates/merman-render/src/family/preparation.rs` constructs only those two Block plans.
- `docs/rendering/diagram-theme-coverage.md` records Block as partial typed support with a
  remaining family bridge.
