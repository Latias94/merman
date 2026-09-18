# XY Chart axis-label glow from the actual reference scene

Baseline: `b205f03c9` plus this increment.
Plan: [theme product boundaries](../../../plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md), U8.
Status: bounded consumer and public recipe correction; full U8 and C7a remain open.

## Reference finding and public behavior

The controlled-font browser comparison found that Modern Mermaid's generic `.label` rule also
matches XY category and numeric label groups. Its location under the Flowchart source comment
does not restrict the CSS selector to that family. The fixed reference commit is
`a021cbce37fc0b07a9f4791c28e983101ea06f2d`; `src/utils/themes.ts` has SHA-256
`9f1baf22457de1fbc5217437dc103840a14f15f2213eeaba45f91e306f6db92d`.

With Mermaid 11.17.2, all four category labels and eleven numeric labels in the fixed XY scene
have cyan fill, weight 600 and `rgba(0, 242, 255, 0.5) 0 0 10px` text shadow. The preceding Merman
public preset produced weight 400 without a filter. The source rule does not specify font size;
14px is the observed default, not a new preset override.

Static AxisLabel rules and effect bindings now use the existing measured-text effect consumer,
writer observations and native ink containment checks. The public Cyberpunk recipe requests
weight 600 and a zero-offset, zero-spread sRGB shadow with sigma 5 and alpha 0.5. It leaves label
size unspecified. Clear disables the effect; source label colors and configured sizes retain
their existing ownership. Matching ordinal effects and unsupported sibling facets retain
residuals. Support discovery advertises conditional AxisLabel effects.

The builtin recipe fingerprint is
`7e88ad763d457e83bb59bcc861d55865310184af9cf398a6cdcc65722dd9cea7`.
Schema and recipe revision remain 1; qualification cells remain empty. No new dependency,
production font asset, selector field or proof framework was introduced.

## Controlled-font capture contract

The local experiment supplies the same Arial regular and bold font files to Chromium and to the
public native API's caller-owned embedded-only font catalog:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| Arial.ttf | 773236 | `525979822591a3447cfc49d943d6f7683508e25543407871c0ed8fed05fd2bd9` |
| Arial Bold.ttf | 750984 | `d72db21f9242aedd6b917d8549ad5921766b24d5f8d0becfda2ff4c620b3c2e0` |

The font bytes remain local ignored experiment inputs. They are not bundled or distributed by
the library. This caller-supplied profile cannot promote the resource-free builtin recipe to
Portable on arbitrary hosts.

Chromium runs at 1280 by 960, DPR 1, after font readiness, with one SVG unit per CSS pixel. The
reference uses the pinned recipe and its preview background; this is not a complete React
Preview application run. Raw reference XY SVG has an opaque diagram background, while Merman
emits the plan's self-contained root canvas. The fourth reference line lacks the first three
explicit series CSS rules; Merman intentionally cycles its typed palette. Neither difference is
normalized away or claimed as whole-image parity.

Baseline artifacts are under `target/bench/experiments/cyberpunk-xy-scene-b205f03c9/`; the updated
capture is under `target/bench/experiments/cyberpunk-xy-axis-label-b205f03c9/`. Both include recipe,
font provenance, actual SVG/PNG/PDF and browser observations. Temporary capture sources are
retained with the experiment; native captures use a freshly built public facade with
`svg,png,pdf,layout-cytoscape,embedded-fonts` and no defaults.

## Verification

The public preset regression failed before the production change: expected weight 600, observed
no explicit weight. A subsequent compile caught the new integration test trying to call a
private compiled-theme accessor; the test now builds its specification through a shared test
helper instead. Neither failure was treated as successful verification.
The first broad run also caught a stale series-only native filter count (6 rather than 19).
That witness now counts its four bars, two lines, two category labels and eleven numeric labels
explicitly, retaining exact filter/stage/reference and PNG/PDF receipt equality checks.

Final Release nextest passed **2,746 tests with two skipped**, covering renderer/facade libraries,
XY Chart and support-discovery integration tests, and facade text, series, canvas and composed
effects. The command used `CARGO_BUILD_JOBS=1`, shared target, no defaults and
`merman/svg,merman/png,merman/pdf,merman/layout-cytoscape`; output is
`/tmp/merman-xy-axis-label-final.log`. This was not a full workspace or platform-package run.
`cargo fmt --all -- --check` and `git diff --check` passed.
Clippy on the two changed libraries passed with existing warnings under the same profile plus
`merman/embedded-fonts`; output is `/tmp/merman-xy-axis-label-clippy.log`.

The actual simplify pass used one independent Codex context for reuse, quality and efficiency
under the thread-capacity limit. Its one accepted suggestion merged identical static XY text
facet guards without broadening selectors. Actual `ce-code-review mode:agent` run
`20260919-052730-4e5a5e6f` completed with no actionable source findings. The run identifier uses
the local September 19 clock. Six lenses ran serially in one independent context, not six
independent agents. Its initial pass missed the old filter-count assertion; the real regression
found it, and the reviewer subsequently inspected the correction. The reviewer did not run
Cargo or repeat artifact captures. The parent performed those checks.

## Actual artifacts and an open budget gate

The fresh embedded-font facade build produced the caller-font recipe fingerprint
`30ba9ebf78aa18881b89e65971e8f83bada67f8cd05094b0d8aecfd0568787d8`. Both default PNG and default
PDF exports rejected the complete fixed scene with `max_total_svg_conversion_filter_primitives`:
actual 130 at the stopping point, maximum 128. This is a correctly returned resource error;
the first temporary probe panicked only because it unwrapped that error.

For diagnosis only, the experiment explicitly raised this one caller option to 256, preserving
all other default conversion limits. Both completed conversion plans contain **28 filtered
groups and 140 filter primitives**, 158 tree nodes, depth 6, isolation depth 2, 20 subroots and
no nested SVG images. Both resulting admissions are Portable with Embedded fonts and no reasons.
That statement applies only to this explicit font/resource profile. The library default remains
128. This complete public-preset scene therefore **does not yet pass native export with default
options**; the smaller passing integration scene has 19 filters and fits the default ceiling.

Chromium 151.0.7922.34 verifies all fifteen category/numeric labels have weight 600, the reference's
14px default size and cyan paint, with a real filter binding. The baseline had weight 400 and no
filter. Both reference and Merman report custom ArialMT/Arial-BoldMT through CDP. PNG and actual
PDF rasterized at 96 dpi with the existing PDFium library were visually inspected: labels are
readable and glowing, with no observed crop in this scene. PDFium library SHA-256 is
`6dbc4ceaa40178e3b583a51144cccd7900a19608fd71f45ecca4a6c766d8024b`.
The PDF raster is 1011 by 755 because of point-to-pixel conversion; it is not a same-pixel parity
comparison with the 759 by 566 PNG.

The next U8/U10 action must measure the cost of the actual 140-primitive scene and representative
larger scenes, then decide whether to reduce repeated work or justify a bounded default budget
change. The experiment's 256 is not a recommended new default. No acceptance threshold or public
qualification cell was changed to hide the rejection. U8 remains open for this concrete gate as
well as complete scene qualification.

## Limits

This correction does not close scene qualification, perceptual tolerances, actual installed
Web/WASM freshness, the full platform matrix, or U10 performance and size measurements. Native
export still rejects effects whose actual text ink exceeds the bounded measurement reserve.
