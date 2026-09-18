# Public Cyberpunk XY text glow

Date: 2026-09-19 (local). Baseline: `385c48fa3` plus this increment.
Plan: [theme product boundaries](../../../plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md), U8.
Status: public recipe wiring and bounded host observations complete; full U8 and C7a remain open.

## Public behavior

The public Cyberpunk recipe now applies independent sRGB text shadows to XY Title, AxisTitle
and Legend. The fixed Modern Mermaid source at `a021cbce37fc0b07a9f4791c28e983101ea06f2d`,
`src/utils/themes.ts:366–381`, declares CSS text-shadow blur radii 15, 10 and 8 with cyan alpha
0.8, 0.6 and 0.5 respectively. The existing shadow representation uses Gaussian sigma, so the
recipe uses 7.5, 5 and 4, as established in the [design investigation](2026-09-18-xychart-text-glow-design.md).
Opaque cyan glyphs retain their existing sizes and weights. These rules are scoped to XY Chart;
axis tick labels and other families do not acquire these effects. No public DSL, dependency or
font resource was added.

The actual compiled recipe fingerprint is
`932a54d67b0bec2e8e9f3964bff5b3b7c203085c587b4b49fb13a613ca16fcb5`.
The catalog records that value. Unpublished schema/revision values remain 1, resource identity
is unchanged and qualified cells remain empty. Recipe identity changes do not promote visual
qualification or broaden the public design scope.

## Verification

The existing public preset/export-import test first failed with `public role glow must be bound`.
After wiring the recipe, all four XY text tests passed with no embedded-fonts feature. The test
now covers English and Chinese in both orientations, actual text/filter references, sRGB,
role-specific sigma/alpha, unchanged tick text and identical direct/imported SVG and PNG output.
Native checks reject incomplete filter receipts and unlocalized PDF filters. Final admission
must explicitly be Portable or HostDependent; Unverified/Rejected and future unknown states
cannot silently pass that witness.

The final renderer Release library run passed 2,584 tests with two skipped, including catalog
fingerprint reconciliation, resource admission and other family/preset regressions.
Final facade Release regression passed 16/16 across `theme_xychart_text`, `theme_xychart_series`,
`theme_canvas_background` and `theme_composed_effects`, with
`--no-default-features --features svg,png,pdf,layout-cytoscape`. One existing State composed-shadow
test received nextest's LEAK marker in that run; immediate isolated rerun passed 1/1 without the
marker. This records the observation and does not establish its root cause or a memory-leak diagnosis.
`cargo fmt --all -- --check`, `git diff --check` and renderer-library Clippy succeeded. Clippy
still emitted the same 170 warnings recorded in the preceding consumer increment; no warning-free
claim is made. Builds used shared target and `CARGO_BUILD_JOBS=1`.

The actual `ce-simplify-code` reuse, quality and efficiency review completed serially in one
independent Codex context after thread capacity prevented further delegation. It found no
beneficial simplifications. Existing shadow construction, scoped rules and native helper were
reused; no observation framework was added.

## Actual artifact observations

A temporary Rust probe linked the freshly built `merman` Release rlib and used only the public
compile_preset/render/document/export APIs. It produced SVG, PNG and PDF for the fixed
`public-cyberpunk/xychart.mmd` fixture and separate Chinese vertical/horizontal scenes with
named series. The extra Chinese scenes supplement, rather than replace, the fixed fixture.
All six native receipts were HostDependent with only SystemOrHostFontDependency. They did not
claim resource-backed Portable text. Native PNG inspection showed readable Chinese glyphs,
role glows, series paint and the complete grid/radial canvas without observed edge clipping.

Chromium `151.0.7922.34`, at 1280x960 CSS pixels, DPR 1, loaded those actual SVGs after font
readiness. Filtered text was Request Volume/Window/Requests in the fixed fixture and
请求统计/销售/月份/数量 in the Chinese scenes. Captures retained the visible effects in both
orientations. This probe does not claim a newly built WASM package or Playground integration.

The actual PDFs were independently rasterized with the existing host application's PDFium
library, SHA-256 `6dbc4ceaa40178e3b583a51144cccd7900a19608fd71f45ecca4a6c766d8024b`,
at 96 dpi. The probe added no project dependency. Chinese output visibly retained glyphs,
text shadows, translucent bars, borders and the grid. Raster sizes were 1011x755 (fixed),
995x755 (Chinese vertical) and 995x757 (Chinese horizontal); the PDF page uses points and these
captures are not scale-1 pixel comparisons with PNG. Previously observed PDFKit differences
are not resolved by this PDFium observation.

Artifacts and machine-readable browser/PDFium observations are retained under
`target/bench/experiments/cyberpunk-xy-text-385c48fa3/`. Captures use system/host font resolution;
no common font-file identity was established across browser/native, so they are host observations,
not cross-target typography qualification or whole-image parity. No new reference-Mermaid capture
or full visual tolerance matrix is claimed.

## Independent source review

The actual `ce-code-review mode:agent` run `20260918-201310-c6f80533` completed with no actionable
findings against the four Rust files and two documents. Source-set SHA-256:
`51de7edca27fc61c060bb94fd9798d646e3f2e51700463b69f5d8ce5be6ce703`.
An independent Codex context reviewed correctness, testing, maintainability, API contract,
adversarial behavior, project standards and performance serially under the thread-capacity limit.
This was not seven independent reviewers. The parent executed the tests; the reviewer did not
rerun Cargo. The review confirmed that the reference tick-path opacity remains absent from the
captured output, and retained this as unfinished complete-scene work rather than a claim that
text-only progress qualifies U8. Existing no-common-font and viewer-specific limits also remain.

## Remaining acceptance

U8 still needs complete scene comparison against the pinned reference, including remaining
axis/tick semantics, terminal visibility and clipping checks with declared font provenance.
Reference tick-path opacity is a separate requirement from these text effects. U9 still owns
public profile/qualification promotion and installed transport rollout. U10 still owns measured
filter allocation, latency, memory and artifact-size impact. The earlier bounded one-em filter
reserve is neither an ink guarantee nor a zero-cost claim.
