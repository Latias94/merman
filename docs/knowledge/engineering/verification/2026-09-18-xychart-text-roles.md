# XY Chart text roles and typography

Date: 2026-09-18. Base: `611f474f2`.
Plan: [theme product boundaries](../../../plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md), bounded U8 increment.
Status: scoped implementation and verification complete. U8 and C7a remain open.

## Behavior and boundaries

New `axis-title` and `axis-label` targets are limited to XY Chart. Together with Title and
Legend, their static unqualified/Default rules consume font size, font weight and solid,
transparent or cleared fill. Sizes and weights enter layout measurement before painting;
the writer consumes the same role weight. Existing paint receipts bind the actual emitted
text, fill, size and weight. No public layout wire fields or dependencies were added.

Text paint is inherited by Title, axis text and Legend. Axis text also inherits Axis, with
all properties merged by original author order. Legend markers retain series ownership;
point labels retain their existing separate paint behavior. Generic Text/Axis typography
remains unsupported when it wins on actual text. Ordinal role typography remains unsupported.
Explicit role font-size configuration owns only size; it does not suppress theme weight or
paint. Clear restores the configured baseline and is verified through the actual terminal.

The source-size flags are independent of optional role-font resolution. An absent Legend rule
cannot change whether a Text size request is owned by explicit title configuration. There are
only six private logical roles; the optional font cache does not grow with chart size. Existing
terminal receipt string copies remain; this increment makes no performance or memory claim.

Public target discovery and the independent support manifest include both target IDs and the
bounded role facets. Protocol revisions remain 1. The public Cyberpunk recipe, its fingerprint
and empty qualification cells are unchanged: this increment implements consumers before adding
new preset role rules or text glow.

## Observed failures and corrections

- The measurement probe initially observed 20px instead of the requested Title 18px. The final
  layout resolves role typography before measuring and writing.
- The source-ownership regression failed with `UnverifiedFamilyTheme` for a Text size request
  overridden by explicit title size. Its result formerly depended on an unrelated absent
  Legend font rule. Source-size flags now resolve independently.
- The first broad run passed 2,614 and failed four tests: two support-manifest assertions,
  public discovery, and an old Text-to-Legend isolation assertion. The manifest and old
  expectation were synchronized; point-label isolation remains covered.
- The four-role fill Clear regression failed with `IncompleteFamilyTheme` (two required
  mechanisms, one accounted). The existing static paint helper intentionally returns no
  replacement color for Clear. The local accounting path now retains and verifies the actual
  configured baseline instead of treating the missing replacement as an incomplete consumer.
- The next broad run passed 2,620 tests and failed only the Axis-font override case:
  generic Axis typography still produced a residual on a path after all axis text had been
  overridden by role rules. Paths now ignore typography facets; uncovered tick text still
  retains its unsupported generic winner. The test includes both complete and partial override.

Local red logs: `/tmp/merman-xy-text-red.log`, `/tmp/merman-xy-source-red.log`, and
`/tmp/merman-xy-clear-red.log`. The Axis failure is recorded in
`/tmp/merman-xy-text-second.log`. These are development evidence, not release receipts.

## Validation boundary

The new public facade test writes a typed `ThemeRecipeV1`, imports it into a fresh compiler,
and compares role attributes and native output. This is saved authoring-recipe exchange.
There is no arbitrary compiled-theme export API; only built-in preset export exists, and this
test does not claim to exercise a nonexistent canonical reverse compiler. Existing public
Cyberpunk preset-export tests remain separate.

The final locked Release renderer library, XY SVG and support-discovery run passed **2,621**
tests with **2 skipped**. The public facade run passed **10/10** with
`svg,png,pdf,layout-cytoscape` and no default features, including the new typed role recipe,
actual role-colored PNG pixels, PDF output/admission and equal pixels after recipe exchange.
Existing public Cyberpunk series and composed-effect tests also passed. The binding metadata
selection passed **22/22** (264 unrelated tests filtered out). Renderer Clippy completed with
existing warnings and no diagnostics in the XY implementation. `cargo fmt --all -- --check`
and `git diff --check` passed.

No full workspace, browser matrix, installed package matrix, rasterized PDF visual comparison
or performance baseline is claimed here.

Executed checks, serially with the shared target directory:

```text
CARGO_BUILD_JOBS=1 cargo nextest run --release --locked -p merman-render \
  --no-default-features --lib --test xychart_svg_test \
  --test theme_support_discovery_test --no-fail-fast
CARGO_BUILD_JOBS=1 cargo nextest run --release --locked -p merman \
  --no-default-features --features svg,png,pdf,layout-cytoscape \
  --test theme_xychart_text --test theme_xychart_series --test theme_composed_effects
CARGO_BUILD_JOBS=1 cargo nextest run --release --locked -p merman-bindings-core \
  --no-default-features --features svg,png,pdf,layout-cytoscape --lib \
  -E 'test(metadata::tests::)'
CARGO_BUILD_JOBS=1 cargo clippy --locked -p merman-render --no-default-features --lib
```

Final local logs: `/tmp/merman-xy-text-final.log`, `/tmp/merman-xy-text-facade.log`,
`/tmp/merman-xy-text-bindings.log`, and `/tmp/merman-xy-text-clippy.log`.

## Review and simplification

An independent Codex context completed `ce-code-review` with no remaining actionable findings:
`/tmp/compound-engineering-501/ce-code-review/20260918-170652-12fe8893/review.json`.
Thread capacity prevented separate reviewer contexts; seven lenses were evaluated serially in
that one context, not by seven independent reviewers. The receipt preserves the three observed
correctness failures above and their source fixes. Runtime verification is owned by the root
agent and is reported separately; the reviewer ran no Cargo commands.

The reuse, quality and efficiency rubrics found no need for additional abstractions. A small
simplification removes the redundant `line` argument from accounting: axis text now has concrete
roles, so only axis geometry reaches that method with target Axis. The reviewer checked all
three call sites. Existing per-facet filtering, work metering and terminal validation remain.

## Remaining work

Add the reference-backed role recipe and bounded text glow to public Cyberpunk; then validate
complete public scenes, including tick paint and background composition. U6/U7/U8 completion,
U9 qualification, U10 cost comparison and C7a delivery remain open. The earlier three findings
against `54f8398f4..6d9ef7c80` were fixed by later commits; see
[their disposition](2026-09-18-theme-admission-and-edge-effects.md).
