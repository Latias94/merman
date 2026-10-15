# Public Flowchart text glow and typed background ownership

Date: 2026-09-19. Baseline: `1eec22970` plus this increment.
Plan: [theme product boundaries](../../../plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md), U6.
Status: bounded implementation, independent review and final scoped verification complete.
U6 and C7a remain open.

## Public behavior

The public Cyberpunk recipe now binds its existing sRGB five-pixel text-shadow graph to
Flowchart NodeLabel and EdgeLabel. These scoped rules preserve weight 600 and the existing
separate Node and Edge graphs. The fixed Modern Mermaid source at
`a021cbce37fc0b07a9f4791c28e983101ea06f2d`, `src/utils/themes.ts`, assigns a ten-pixel CSS
text-shadow blur to `.label` and `.edgeLabel`; the existing lowering uses Gaussian sigma 5.
No new effect model, dependency, embedded font or public schema was introduced.

The compiled recipe fingerprint is
`af00e9dd770d06f8aaea3c85332f184800a2e5a9623d6de0607205a0812fb6f6`.
The catalog records this content identity; unpublished revision values remain 1 and public
qualification cells remain empty. The public recipe exchange regression exercises actual JSON
serialization, fresh compilation and both HTML/native text modes.

## Background ownership and remaining bounds

A typed EdgeLabelBackground paint now owns one layer and its requested alpha. Previously,
classic native rectangles multiplied alpha by 0.5, while the ordinary HTML div/paragraph pair
composited an authored alpha 0.4 to 0.7. Transparent could become visible black at alpha 0.5.
The shared CSS now clears duplicate typed container/paragraph paint, retains the real native
rectangle at opacity 1 and paints ordinary HTML in its natural labelBkg div. Icon/image label
CSS follows the same ownership distinction. Swimlane retains its existing actual background
rectangle. Explicit Mermaid configuration keeps its historical rendering and source precedence;
the shared color utility was not changed.

Ordinary HTML with typed background and nonzero EdgeLabel padding cannot certify its padded
background extent from host layout estimates. Its natural HTML background and text position
are retained without the duplicate estimated rectangle. The winner remains a residual and
RequirePortable rejects the combination. Native SVG padding remains supported. Clear remains
its existing separate unsupported request; it is not reinterpreted as Transparent. This is a
bounded supported surface, not a new general HTML box-measurement contract.

## Mixed-terminal reconciliation

Independent review found that the existing per-rule set of successful terminals could hide
an unverified padded HTML edge when the same rule also painted an icon/image label. An actual
mixed probe incorrectly returned only host-font dependence. Background receipts now accumulate
completion with logical AND for every winning fill occurrence. A successful sibling cannot
clear an earlier failure; source-owned and absent terminals retain their existing ownership.
The padded-HTML test includes an icon before and after the edge in source order and requires
one residual and strict rejection in both cases. Native text remains accepted.

## Browser callback boundary

The first fresh Web run exposed a real consumer gap in both text modes: shape and edge glows
were present, but glyph glow was absent. The effect planner had used the private built-in
measurement carrier, which authorizes reuse of an operation, as a drawing admission condition.
Browser callbacks intentionally have no such identity because their order, failures and
provenance must remain observable.

Ordinary HTML now allocates the requested effect from existing finite layout dimensions while
retaining its source, markup and writer checks. Native SVG keeps prepared ink/allocation bounds
when available; a bounded plain-text fallback uses centered layout coordinates, the first
baseline at one em and the existing one-em paint reserve. Explicit markup, hard breaks, math,
empty text and invalid dimensions do not enter that fallback. Automatic wrapping remains the
writer's responsibility; this allocation is not a guarantee of line count or final font ink.
No carrier, prepared binding, callback cache or measurement protocol changed. The optional
native edge effect requests its y offset through the existing routed measurer and compares it
with the actual writer translation.

The underestimated-host regression now requires an actual filter in both text modes and a
NativeFilterReceiptMismatch from both PNG and PDF. It preserves the target rejection while
allowing the requested SVG drawing. A separate stateful offset regression requires observable
geometry requests, no wrongly positioned EdgeLabel filter and incomplete theme evidence.
The final focused run after this change passed 260/260. Host-dependent allocation is not a
Portable font guarantee, and native containment checks remain mandatory.

## Review

The simplify pass used one independent Codex context for the reuse, quality and efficiency
rubrics. Its only accepted suggestion used the existing `InsetsPx::all` constructor in a test.
The actual `ce-code-review` run `20260919-212347-60a60f60` used six serial lenses in one
independent Codex context, found the mixed-terminal defect above, and rechecked its correction.
Its follow-ups also corrected the browser test to inspect the actual SVG/HTML filter owners
and reviewed the callback allocation change against the final source hashes. It retained no
final source defects. The review receipt records runtime gates as pending at
review time; the parent verification addendum records their final outcomes without overwriting
that historical verdict. No external model or multiple independent reviewers are claimed.

## Verification

The strengthened public recipe test first failed because the actual text had no glow binding.
The new native pixel regression first measured alpha 51/255 for a requested 0.4; the corrected
matrix expects 101–103/255 across Flowchart/Swimlane, HTML/native text and zero/nonzero padding.
It also checks that only the stated ordinary padded-HTML case retains incomplete theme evidence.
The fixture uses an explicitly transparent canvas and transparent Swimlane cluster background
so its alpha assertion observes uncomposited red paint rather than a colored backdrop.

The pre-review renderer/facade Release run passed 2,797 tests with two existing skips, across both
libraries and `flowchart_node_effects`, `flowchart_svg_test`, `theme_flowchart_edge_effects`,
`theme_flowchart_geometry`, `theme_flowchart_markers` and `theme_canvas_background`. A final
focused alpha test also passed after the test used the existing `InsetsPx::all` constructor.
After the mixed-terminal receipt correction, seven integration targets, including
`block_svg_test`, passed 259/259. After the callback allocation change and its stateful-host
negative, those targets passed 260/260 on the final source. The two original HTML structural
class/font-weight regressions also passed separately (five unrelated tests filtered out). Builds used shared target, `CARGO_BUILD_JOBS=1`, `--locked`, `--no-default-features` and
`merman/svg,merman/png,merman/pdf,merman/layout-cytoscape`.

A fresh native probe used only public APIs with the fixed `public-cyberpunk/flowchart.mmd`
fixture. Both HTML and native text modes have 13 actual native filter receipts in PNG and PDF.
Direct preset and freshly compiled exported recipe produce identical SVG and PNG. Native
admission is HostDependent with only SystemOrHostFontDependency; no embedded-font or portable
font claim is made. JSON round-trip identity is independently covered by the checked-in test.

Chromium 151.0.7922.34 passed ten actual SVG probes: public HTML/native scenes, alpha 0.4 with
and without text glow, Transparent, zero-alpha red, padded alpha with/without glow, Swimlane and a mixed icon/padded-edge scene.
Ordinary HTML backgrounds track actual natural paragraph boxes (59.984×24, 50.594×48 and
200×96 in the short/hard-break/automatic-wrap fixture). Paragraph backgrounds are transparent,
requested div alpha remains exact, and disabling text filters changes actual screenshot bytes.
Padded HTML observations prove preservation of natural content coverage, not the unverified
padding extent. Swimlane observations record its existing rectangle, not a general guarantee
that arbitrary host HTML metrics match it.

The final full Web WASM transaction is `58420c448ba4`, freshly built after the callback fix
with `CARGO_BUILD_JOBS=1` and `BINARYEN_CORES=1`. TypeScript compilation, package assembly,
WASM source-input verification and the full package smoke (35 diagram kinds) passed. Playground
production build and its artifact graph check passed. Two Chromium desktop tests passed using
**browser** measurement through the public UI: native SVG and HTML labels, actual shape/edge/text
filter application, glyph-only background ownership, and canvas color/gradient geometry/layer
order. Disabling the relevant effects changes actual screenshot bytes. Test TypeScript checking
and scoped ESLint passed. The preceding fresh Web run failed both cases because of the
built-in-carrier restriction; that red result is retained separately from the final passing run.
Only the full WASM profile was rebuilt here; this is not an all-profile installed release matrix.

The exact full SVG structure gate passed all 35 comparison commands with
`--check-dom --dom-mode structure --dom-decimals 3 --diagnostic-browser-text-layout`.
Scoped Release Clippy passed with the same existing warning messages as the preceding run;
this is not a zero-warning claim. `cargo fmt --all -- --check` and `git diff --check` passed.
All Cargo builds ran sequentially against the shared target directory.

Local artifacts:

- `/tmp/public-flowchart-text-glow-red.log`
- `/tmp/flowchart-typed-background-red.log`
- `/tmp/flowchart-background-mixed-red.log`
- `/tmp/flowchart-public-text-final-tests.log`
- `/tmp/flowchart-public-text-final-alpha.log`
- `/tmp/flowchart-public-text-reviewed-tests.log`
- `/tmp/flowchart-public-text-host-final-tests.log`
- `/tmp/flowchart-public-text-host-weight-tests.log`
- `/tmp/flowchart-public-text-host-native.log`
- `/tmp/flowchart-public-text-host-browser-probes.log`
- `/tmp/flowchart-public-text-web-browser.log` (red browser callback result)
- `/tmp/flowchart-public-text-host-web-build.log`
- `/tmp/flowchart-public-text-host-playground-build.log`
- `/tmp/flowchart-public-text-host-web-browser.log` (final two passing UI tests)
- `/tmp/flowchart-public-text-host-web-typecheck.log`
- `/tmp/flowchart-public-text-host-web-eslint.log`
- `/tmp/flowchart-public-text-host-dom.log`
- `/tmp/flowchart-public-text-host-clippy.log`
- `/tmp/flowchart-public-text-host-fmt.log`
- `/tmp/flowchart-public-text-matrix/summary.json` and adjacent SVG/PNG/PDF files
- `/tmp/flowchart-public-text-native.rs` (public native probe)
- `/tmp/flowchart-public-text-browser.mjs` (browser probe)

These are session artifacts, not release receipts. Full reference visual comparison, controlled
font provenance, all presets/families, installed-profile matrices and U10 cost evidence remain
open. This increment does not promote qualification cells or freeze the public contract.
