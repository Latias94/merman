# XY Chart Series Paint (U8 Increment)

Date: 2026-09-18. Base: `2521563e2`.
Plan: [theme product boundaries](../../../plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md), U8.

## Delivered boundary

The XY Chart series owner resolves static, Default, Exact and Cycle selectors without requiring
an ordinal palette. Series ordinals follow source declaration order across plot kinds. Actual
bar/line geometry and visible legend markers consume solid/transparent/Clear fill and stroke,
stroke width, and whole/fill/stroke opacity. Point-label palette paint stays separate from
geometry fill and alpha. Source and site palette ownership release color obligations while
width and opacity remain independently applicable.

Clear restores the family/configuration baseline and blocks that property's palette fallback.
Absent plots, shadowed rules and unmatched selectors remain NotApplicable. Unsupported winning
siblings remain residual even when a supported facet of the same rule reaches its writer.
Receipts check actual emitted width, alpha, colors, plot/mark order and visible legend terminals.
Public support discovery declares these six facets TypedPartial; the unpublished manifest stays
at revision 1. No dependency, public schema or preset qualification cell changes are included.

The family-local series module replaces the old palette-only owner. It does not introduce a
shared proof abstraction. Layout records visible per-kind legend indices once; writer lookup is
constant-time, and membership checks use sorted indices. Requests without a series obligation
avoid extra legend index allocation and per-plot style resolution. Route traversal is charged to
the work meter. These are implementation observations, not measured performance gains.

## Verification

All runtime checks below use locked Release builds with serial Cargo compilation.

| Check | Result |
| --- | --- |
| Renderer lib + `xychart_svg_test`, no default features | 2548 passed, 2 skipped |
| Final facade SVG/PNG/import, typed render and layering | 11 passed |
| Renderer library Clippy, no default features | Passed with existing warnings; no XY Chart warnings |
| `cargo fmt --all --check` and `git diff --check` | Passed |

Renderer command:

```text
CARGO_BUILD_JOBS=1 cargo nextest run --release --locked -p merman-render \
  --no-default-features --lib --test xychart_svg_test --no-fail-fast
```

Facade command:

```text
CARGO_BUILD_JOBS=1 cargo nextest run --release --locked -p merman \
  --no-default-features --features svg,png --test theme_xychart_series \
  --test xychart_typed_render --test diagram_theme_layering
```

The new facade tests compare actual SVG attributes, import a complete recipe into a fresh
renderer, and inspect PNG pixels. The PNG control compares quarter-alpha red interiors over
black with opaque red, while green borders and a separate blue line retain their pixel counts;
a zero-width control removes the green border.

The initial public facade regression failed before implementation: a requested red bar still
used the default `#ECECFF`. Subsequent regression exposed two issues fixed in this increment:
opacity must preserve f32 values rather than use the geometry formatter's near-integer rounding,
and public support discovery must be updated alongside the private mechanism matrix. Alpha
coverage includes 0, 1e-8, 0.2, 0.9999995 and 1. The first complete renderer run passed 2547 tests
and failed only the support-manifest drift assertion; its result is not treated as a passing gate.

## Review and remaining work

Review receipt:
`/tmp/compound-engineering-501/ce-code-review/20260918-221243-f665eafa/review.json`.
One independent Codex reviewer covered correctness, testing, maintainability, project standards,
performance, API contract and adversarial cases, including the final discovery change. No
remaining actionable finding was retained. Thread capacity prevented additional reviewer
contexts; the three simplification lenses were completed inline. These are not seven independent
reviews or a cross-model review. Test execution is owned by the verification results above.

U8 remains open: plot-kind-specific recipe selection, effects, typography and the complete
Cyberpunk SVG/PNG/PDF scene still require implementation and validation. The public Cyberpunk
recipe is unchanged. This increment does not claim fresh browser, PDF, installed-package,
all-platform, artifact-size, throughput or memory evidence. C7a and the overall goal remain open.
