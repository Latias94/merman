# XY Chart Series Effects (U8 Increment)

Date: 2026-09-18. Base: `21a13096e`.
Plan: [theme product boundaries](../../../plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md), U8 / R1–R3 / KTD4–KTD6.

## Boundary and initial evidence

The renderer connects the existing bounded zero-spread shadow sequence to actual XY Chart bar/line geometry
and visible legend markers. It preserves effect rule/Clear/binding precedence, global declaration
ordinals, source-owned colors, separate labels, resource admission and final target evidence.
It reuses the shared shadow lowering, writer and native receipt owners. No new dependency or public
schema is required. The public preset recipe remains unchanged until its complete scene passes.

Before implementation, the public facade regression with two bars and a flat line compiled and
rendered, but PNG admission reported ThemeEvidenceIncomplete. The companion complete-recipe
import test successfully read two sRGB shadows, a ChartSeries binding and an Exact(2) effect
Clear, then failed because it emitted zero filter references instead of the expected three
(two bars and their visible legend marker). These are observed failures, not inferred gaps.

## Verification

The final renderer run covers the completed source, including the additional rule-accounting
test and cost edits. All runtime checks use locked Release builds with serial Cargo compilation.

| Check | Result |
| --- | --- |
| Final renderer lib + XY integration, no default features | 2553 passed, 2 skipped |
| Facade composed effects and XY series, SVG/PNG/PDF without embedded fonts | 7 passed |
| Renderer Clippy, no default features | Passed with existing warnings; no XY Chart warning |
| `cargo fmt --all --check` and `git diff --check` | Passed |

Each filter uses actual serialized mark geometry and an admitted user-space region. Bounds
include stroke and shadow outsets without changing layout coordinates. Only requested effects
trigger path-bound calculation. A failed materialization still contributes to the expected
application count; missing definitions or references cannot pass native reconciliation.

## Review and simplification

One independent Codex context completed seven review lenses with no retained source finding:
`/tmp/compound-engineering-501/ce-code-review/20260918-225918-517d4391/review.json`.
The reviewer ran no Cargo or browser checks; runtime evidence belongs to the table above.
The initial review missed the native ID convention. Its updated receipt preserves that miss
and records the subsequent source-level export-chain recheck; runtime verification found the bug.
Subagent capacity prevented separate simplification reviewers, so the three loaded reuse,
quality and efficiency rubrics were applied inline. The applied changes reuse graph lowering
by ID, avoid a temporary binding-user vector, retain only a boxed optional shadow per node,
and charge geometry plus shadow-stage work. No safety check or evidence owner was removed.
These are structural observations, not measured throughput or memory improvements.

The first facade run passed six checks and exposed a native export integration omission:
XY filter IDs lacked the existing `-theme-effect-` marker consumed by the native observer.
The writer now uses the same convention as the other effect consumers. The existing observer
and receipt comparison were kept strict. The subsequent 7/7 native/facade run passed, including
linearRGB and sRGB, Previous versus SourceGraphic inputs, PNG color counts, native/PDF matching
filter receipts and the localized PDF admission check. It does not claim rasterized-PDF visual
comparison or complete public preset qualification.

Executed commands:

```text
CARGO_BUILD_JOBS=1 cargo nextest run --release --locked -p merman-render \
  --no-default-features --lib --test xychart_svg_test --no-fail-fast
CARGO_BUILD_JOBS=1 cargo nextest run --release --locked -p merman \
  --no-default-features --features svg,png,pdf,layout-cytoscape \
  --test theme_composed_effects --test theme_xychart_series
```

## Plot-kind selection follow-up

An independent read-only audit confirmed that existing ThemeVariant plus string-based wire
fields can express Bar/Line without a second selector language or protocol version. Public
Web variant fields are already strings; discovery and wire decoding derive known values from
the Rust variant catalog. However, Default currently matches exactly: changing XY resolution
to Bar/Line alone would silently drop previously supported Default rules. The follow-up must
preserve XY Default matching and per-property author order, using the existing resolved-style
merge semantics; it must not globally redefine Default or simulate plot kinds with Odd/Even.
No plot-kind contract change is included in this effect increment.

The same pinned-reference audit identified the next text consumers: chart title (18px/700),
axis titles (13px), and legend text (12px), each with its own shadow recipe. Tick labels and
point/data labels do not inherit those role-specific sizes or effects. Current XY typography
consumes FontStack, but not these role-local size/weight/effect winners; Axis currently groups
titles, labels, ticks and axis lines. The next design must distinguish real roles before adding
reference rules, and use identical resolved fonts for measurement and output. Injecting config
outside the saved complete recipe would break export/import identity. The fixed public scene
has no titled series and therefore no legend; any legend extension needs a separate witness.
These are follow-up findings, not new support claims or an approved new selector schema.

## Remaining scope

U8 still requires plot-kind-specific public recipe composition, the required text semantics
and complete Cyberpunk scene validation in SVG, PNG and PDF. A mechanism regression is not
public preset qualification. Browser, installed-package, full host matrix and measured cost
claims require their own evidence. C7a and the overall goal remain open.
