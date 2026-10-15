# Block title route boundary — correction to ceabee68f

The original 2026-09-12 note incorrectly required adding a diagram-title terminal
before retiring Block `Title.fill`. That would introduce unrelated rendering
semantics. The pinned Mermaid renderer does not emit a diagram title, even when
frontmatter contains `title`; composite block labels remain node labels.

## Source and runtime evidence

- `repo-ref/mermaid/packages/mermaid/src/diagrams/block/blockRenderer.ts` emits
  blocks and edges, then sizes the SVG. It has no diagram-title emission.
- `styles.ts` assigns **textColor**, not titleColor, to `.flowchartTitleText`.
  titleColor is used by `.cluster-label` and `.cluster` descendant text selectors.
- `renderHelpers.ts` constructs composite shapes as nodes (`isGroup: false`).
  In the native Block output, the `.cluster` class belongs to the shape, with
  its label as a sibling. The titleColor descendant selectors have no text consumer.
- `crates/merman-theme-acceptance/tests/block_title_legacy_projection.rs` was run
  before production edits at `ceabee68f5a612f06b689539bb07eca41e3e2427`.
  Classic, Neo and HandDrawn each retained composite and leaf labels. None emitted
  `.flowchartTitleText` or `.cluster-label`, and no `.cluster` contained text.
  Twelve solid/transparent, unqualified/Default Title requests left native PNG
  pixels unchanged. A Node.fill control changed the pixels in each profile.
  A separate run on that pre-retirement checkout confirmed all twelve requests
  actually installed one fallback contribution and changed titleColor in effective
  configuration. The old bridge was exercised, not bypassed.

## Retirement

Delete the ineffective Block `title.fill -> themeVariables.titleColor` projection,
including its generic Text fallback. Preserve the source-backed writer and CSS.
Do not add a title model, title paint plan, or positive title receipt.

Classify the former scalar Title routes as Unsupported. The existing family-owned
empty terminal domain records Title requests as NotApplicable; it does not certify
Applied support. Frontmatter, composite labels, and HTML/SVG label mode do not
create a diagram-title occurrence. Explicit Mermaid configuration remains intact.

KTD23 v7 records two historical selector/facet identities and four value probes.
Block's **remaining legacy** inventory changes from 44 to 40 (82 across three
families); the eight already typed Node fill/stroke matrix routes were never part
of that remaining count. Base FontStack/FontSize are typed, not remaining legacy
routes. KTD17's positive typed-route inventory is unchanged.

The new renderer strict-admission test failed before the fix with
`LegacyFamilyThemeCompatibility { residual_count: 1 }`.

## Verification

The implementation passed the full Release renderer suite (3,842 passed, four
skipped), the private acceptance suite (135 passed), and both focused native
retirement tests after adding the explicit absent-bridge assertion. The complete
SVG structure comparison and all five Web package freshness, runtime and DOM
safety smoke checks passed. Formatting and whitespace checks passed.

All five Web profiles still exceed their existing size budgets. This retirement
adds 94 raw bytes to each of full and render relative to the preceding evidence
dispatch build; it is not a size optimization. No budget was raised. These results
do not close C7a or certify a release candidate.

A clean detached checkout of implementation commit
`978ff94008cbb0976d5a30a71dc1757fc84804d8` passed all 86 Block rendering and support
query tests, followed by the complete private acceptance suite (136 passed, none
skipped). The checkout remained clean after both runs. These Release checks used
the pinned toolchain and two build jobs; the checkout reused build artifacts.

```text
CARGO_BUILD_JOBS=2 cargo nextest run --locked --release -p merman-render --test block_svg_test --test theme_support_discovery_test --cargo-quiet --test-threads 2
CARGO_BUILD_JOBS=2 python3 scripts/run_theme_acceptance.py nextest run --locked --release -p merman-theme-acceptance --cargo-quiet --test-threads 2
```
