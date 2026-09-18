# XY Chart tick paint and public Cyberpunk recipe

Baseline: `7d024e11f` plus this increment.
Plan: [theme product boundaries](../../../plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md), U8.
Status: bounded tick consumer and recipe increment; complete U8 and C7a remain open.

## Public boundary

`ThemeTarget::AxisTick` (`axis-tick`) names XY tick geometry, excluding axis lines and tick-label
text. It is a separate target because these terminals coexist with the axis and text roles, and
existing public support queries identify a target and facet. No selector/query field or wire
version was added. Other families report NotApplicable; discovery reports conditional support
for fill, stroke paint and opacity, without advertising generic Axis opacity or tick typography,
stroke width and effects.

Static None/Default solid, transparent and Clear paint and opacity use the existing Axis-to-role
property merge in author order. Source tick colors own only paint. Clear restores source/default
terminals instead of reviving an overridden Axis value; zero opacity remains an intentional
applied value. Matching unsupported ordinals and sibling properties retain residuals. Unmatched
variants and out-of-range ordinals are NotApplicable. The combined Axis geometry ordinal sequence
is preserved alongside an independent tick sequence.

The existing paint receipt now verifies the exact optional opacity attribute emitted by the SVG
writer. Missing, changed and unexpected opacity cannot certify a rule, including Clear. No new
receipt framework or generic CSS handling was introduced. The extra role style is resolved once
per chart when tick rules exist; no-opacity paths skip additional tick classification. Existing
terminal capture still has its previously documented allocation costs; this increment makes no
benchmark or memory-reduction claim.

## Recipe

The fixed Modern Mermaid source `a021cbce37fc0b07a9f4791c28e983101ea06f2d`,
`src/utils/themes.ts:365`, gives tick paths cyan stroke and opacity 0.3. The public Cyberpunk
complete recipe now expresses those properties with an XY-only AxisTick rule. Direct preset
selection and exported/imported recipes use the same consumer. Axis lines and tick text remain
opaque; their other properties and existing role glows are unchanged.

The actual compiler fingerprint for the updated recipe is
`ef41f841eb87ca1ab3f50bd3c47743357592888c3839906d4c92b03e39f585f0`.
Unpublished schema/revision values remain 1, and public qualification cells remain empty.

## Verification

The initial semantic test failed because `axis-tick` was unknown. After implementation, five
focused Release tests passed: role isolation in both orientations; order/source/Clear/absence;
unsupported sibling and matching-selector rejection; combined Axis ordinals; and actual writer
mutation checks. A temporary test type mismatch (`u32` rather than `usize`) was corrected before
that successful run. A Primary variant expectation was also corrected to NotApplicable because
there is no such actual tick terminal; its evidence counts are asserted explicitly.

The public preset test then failed specifically because tick opacity was absent. The recipe was
wired only after observing that failure. The updated witness retains English/Chinese scenes,
both orientations, direct/imported SVG and native export checks, and adds tick/axis opacity
assertions. Final combined Release verification passed **3,025 tests with two skipped** across
the renderer, facade and bindings-core libraries; XY Chart and support-discovery integration
tests; and facade text, series, canvas and composed-effect exports. The run used shared target,
`CARGO_BUILD_JOBS=1`, no default features, and
`merman/svg,merman/png,merman/pdf,merman/layout-cytoscape,merman-bindings-core/svg`.
The complete command/output is retained locally in `/tmp/merman-axis-tick-final.log`.
This includes catalog fingerprint reconciliation, unsupported-target discovery and binding
catalog exposure. It is not a full workspace or platform-package run.

The actual ce-simplify-code pass reviewed reuse, quality and efficiency in one independent Codex
context under the thread-capacity limit. It suggested combining the duplicate inherited Axis
ordinal updates and skipping path classification when opacity is absent; both were applied.
The follow-up pass found no additional simplification in the scoped recipe and public-entry test.
`cargo fmt --all -- --check`, `git diff --check` and Clippy on the three changed packages' libraries
passed. Clippy reported existing warnings (renderer 170, exporter 19, facade 31 and bindings-core 5);
this is not a warning-free claim. Logs are `/tmp/merman-axis-tick-fmt.log` and
`/tmp/merman-axis-tick-clippy.log`.

The actual `ce-code-review mode:agent depth:full` run `20260918-205419-631bc115` completed with no
actionable findings. One independent Codex context covered correctness, testing, maintainability,
API contract, adversarial cases, project standards and performance serially under the capacity
limit; these were not seven independent reviewers. The reviewer read the actual final test
summary, but did not independently run Cargo or repeat the artifact capture. Reviewed Rust source
set SHA-256: `6e34cb748a9e46d734e9998b7f4aad414f2031b3faebebb23a6296b2d003af65`.
The parent subsequently completed Clippy and this verification record; Rust sources did not change.

## Actual artifact observations

A temporary probe linked the freshly built Release facade library and exported public Cyberpunk
SVG/PNG/PDF for the fixed reference fixture and Chinese vertical/horizontal scenes. It asserted
the compiled recipe fingerprint shown above. All six native receipts were HostDependent with
only SystemOrHostFontDependency, with the same host font catalog fingerprint as the preceding
text-glow capture.

Chromium 151.0.7922.34 loaded the actual SVGs at DPR 1 after font readiness. Every tick path had
computed cyan stroke and opacity 0.3, while axis paths retained opacity 1. PNG pixel comparisons
with the preceding increment kept all dimensions unchanged; exactly 210 pixels changed in the
fixed scene, 180 in Chinese vertical and 210 in Chinese horizontal. These are observed image
changes, not a perceptual-parity score or a benchmark. Chinese PNG and horizontal Chromium
captures were inspected for readable labels and the dimmer ticks.

The actual PDFs were rasterized at 96 dpi with the existing host PDFium library, SHA-256
`6dbc4ceaa40178e3b583a51144cccd7900a19608fd71f45ecca4a6c766d8024b`. The horizontal capture was
inspected; text, axes, bars and ticks remained visible. This uses the same viewer as the preceding
bounded observation and does not resolve differences in other PDF viewers or establish common
browser/native font identity. No dependency was installed.

Artifacts, temporary probe sources and observations are under
`target/bench/experiments/cyberpunk-xy-ticks-7d024e11f/`. This is freshly generated native output
observed in a browser, not a freshly installed Web/WASM package or a new reference-Mermaid run.

## Limits

This change does not establish whole-image reference parity, common font-file identity across
hosts, full Web/Node/WASM installation freshness, complete scene qualification, or U10 performance
and artifact-size acceptance. It adds no dependency or bundled font. The previously reported
HTML-weight, raw-JSON and Web color-space findings are already addressed by later commits; their
separate evidence remains in the [admission correction record](2026-09-18-theme-admission-and-edge-effects.md).
