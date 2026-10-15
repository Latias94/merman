# XY Chart Plot-Kind Rules and Public Cyberpunk Series

Date: 2026-09-18. Base: `41160b5d5`.
Plan: [theme product boundaries](../../../plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md), bounded U8 increment.

## Behavior and boundaries

`bar` and `line` extend the existing variant vocabulary. XY Chart resolves its existing Default
rules and the actual plot-kind rules, merging properties by original author order. A later
Default rule can override an earlier kind rule. Ordinals remain one-based across all declarations;
kind and ordinal conditions intersect rather than numbering bars and lines separately. Other
families do not gain plot-kind terminal claims. Wire schema and all unpublished protocol revisions
remain at 1. No dependency or bundled font was added.

The public Cyberpunk complete recipe now includes family-scoped kind/cycle rules and six sRGB
effect graphs. Its three series colors are cyan `#6cc6cb`, purple `#c77dff`, and green `#7ce38b`.
Bars use 20% fill opacity, 2px strokes and 8px shadows at alpha 0.4; lines use 3px strokes and
6px shadows at alpha 0.5. These values come from the XY rules in
`repo-ref/modern_mermaid/src/utils/themes.ts` at `a021cbce37fc0b07a9f4791c28e983101ea06f2d`.
Repeating the three colors beyond the reference's first three selectors is an intentional
extension. Existing source-owned colors retain precedence.

Compiled recipe identity changes to
`97b90ce1e6c8c732e69aef017a3ee5840431f3dc329346d933b33f878d2ee048`.
This is a content fingerprint, not a protocol version or qualification promotion.
Catalog family-design claims and empty public qualification cells remain unchanged.

## Observed failures before implementation

- A complete recipe containing `variant: "bar"` failed compilation with
  `UnknownId { field: "styles.rule.variant" }`.
- The public Cyberpunk facade test produced old palette stroke `#22d3ee` instead of `#6cc6cb`.
- The first broad renderer run passed 2,556 tests and failed four old expectations: State's
  all-variant acceptance test, the old two-graph count, and two fingerprint assertions. The
  corrections explicitly retain Unsupported for State plot-kind rules and synchronize the
  new exact recipe rather than relaxing admission.

## Public-preset composition defect found by native verification

The first public-preset native run passed eight of nine facade tests, but the new PNG/PDF
case reported ThemeEvidenceIncomplete. A renderer diagnostic showed 12 required mechanisms
and 11 accounted: the public recipe's Legend rule was never handled by any XY evidence owner,
even when the actual scene had no legend. All six series filter receipts were already present.

The paint owner now uses the existing unsupported-terminal reconciliation for the final visible
legend labels. No label means NotApplicable; a visible unsupported winner remains a residual;
explicit source-owned legend color suppresses only the corresponding fill obligation. Tests
cover an absent legend, visible legend, disabled legend, nonfitting legend and source override.
This does not add a Legend text consumer or widen its support claim.

## Verification

The final renderer library and XY integration run passed **2,562 tests**, with **2 skipped**,
including the Legend correction, using serial locked Release builds with no default features.
Tests cover author order, Clear and transparency, nonalternating global Exact/Cycle selection,
source ownership,
unsupported sibling residuals and absent-kind NotApplicable. Public facade checks use the actual
preset and a freshly imported saved recipe. The final facade run passed **9/9** tests across
`theme_composed_effects` and `theme_xychart_series`, with `svg,png,pdf,layout-cytoscape` and no
default features. The public recipe emits six single-stage native filters, PNG/PDF receipts
agree, and freshly imported output has identical PNG pixels. This does not certify a rasterized
PDF visual comparison or remove the independent system-font dependency.

The binding metadata suite passed **22/22** selected tests (264 unrelated tests filtered out),
including discovery of both new variant IDs and unchanged preset catalog claims. Renderer
Clippy completed with existing warnings; no warning points to the new XY logic or recipe builder.
`cargo fmt --all -- --check` and `git diff --check` passed.

Executed checks:

```text
CARGO_BUILD_JOBS=1 cargo nextest run --release --locked -p merman-render \
  --no-default-features --lib --test xychart_svg_test --no-fail-fast
CARGO_BUILD_JOBS=1 cargo nextest run --release --locked -p merman \
  --no-default-features --features svg,png,pdf,layout-cytoscape \
  --test theme_composed_effects --test theme_xychart_series
CARGO_BUILD_JOBS=1 cargo nextest run --release --locked -p merman-bindings-core \
  --no-default-features --features svg,png,pdf,layout-cytoscape --lib \
  -E 'test(metadata::tests::)'
CARGO_BUILD_JOBS=1 cargo clippy --locked -p merman-render --no-default-features --lib
```

Local logs: `/tmp/merman-xy-kind-renderer-final.log`,
`/tmp/merman-xy-kind-facade-final.log`, `/tmp/merman-xy-kind-bindings-final.log`, and
`/tmp/merman-xy-kind-clippy.log`. The original native rejection and renderer missing-Legend
report remain in `/tmp/merman-xy-kind-native-diagnostic.log` and
`/tmp/merman-xy-kind-renderer-diagnostic.log`. These are session verification artifacts,
not installed release receipts.

## Review and cost boundary

An independent Codex context completed seven review lenses with no retained source finding:
`/tmp/compound-engineering-501/ce-code-review/20260919-000512-3ef72af9/review.json`.
It performed no runtime checks. The initial static review missed the Legend combination;
actual native verification found it. The follow-up receipt preserves the miss and records the
Legend fix, with no remaining source finding; runtime results are recorded separately above.

The root applied the reuse, quality and efficiency rubrics inline after an actual reviewer
launch failed with the agent-thread limit. The recipe simplification replaces raw kind strings
with existing ThemeVariant IDs; output values are unchanged. The follow-up Legend correction reuses the existing unsupported-terminal
reconciler, rather than adding another evidence abstraction. Resolution reuses `merge_from` and the work meter. Only kinds with applicable rules perform the extra
resolution. No evidence checks were removed. This is not a measured performance improvement.

## Remaining work

XY Chart role-specific title/axis/legend text styling and glow, full public-scene visual
validation, U9 browser entry checks, artifact matrices and cost comparison remain open.
This increment does not qualify full Cyberpunk, complete U8, freeze C7a or certify all targets.
The three findings against `54f8398f4..6d9ef7c80` were already corrected by later commits;
see [their disposition](2026-09-18-theme-admission-and-edge-effects.md).
