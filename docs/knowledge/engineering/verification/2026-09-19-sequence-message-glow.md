# Sequence Message shadows through the public recipe

## Scope

Base: `776d2f0cf`. This increment implements the Message effect surface in U7 of the [theme product boundaries plan](../../../plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md). It does not close U7 or C7a.

The pinned Modern Mermaid Cyberpunk reference applies one zero-offset cyan shadow (sigma 6, alpha 0.6) to `.messageLine0` and `.messageLine1`. These terminals include their endpoint markers. Message text, central connection circles and autonumber decorations retain their separate ownership. The public recipe now applies this existing edge-glow graph to Sequence Message with its existing 2px width. The actual compiled recipe fingerprint is `9bb6f919a8db20e11ef5ce04d15e098067cea39567a9779ab81a8a492ce5c083`; unpublished schema and claim revisions remain 1 and qualification cells remain empty.

Message preparation reuses the existing line/self-path and marker paint bounds. Only a width or effect request creates paint-bound work; default output retains the empty-plan fast path. Actual filters use the shared bounded user-space materializer, including its outward rounding, resource limits and serial shadow semantics. Each writer emits the filter reference on the actual line/path and records its application. Root bounds include those same filter regions. No new dependency, font asset, public model, generic drawable trait or proof framework is introduced.

Static unqualified and Default effect rules, target bindings and explicit Clear are supported. Unsupported selectors, nonzero spread and incomplete terminal emission remain explicit. Discovery is Conditional. Source signal color ownership and requested line width remain independent of the effect facet.

## Verification

- The new binding/rule/Clear integration test failed before implementation with `IncompleteFamilyTheme` (one required mechanism, none accounted).
- Sequence Release SVG regression passed 92 tests, one skipped. Cases cover request/reply, curved and right-angle self calls, separate text ownership, filter bounds, binding precedence, Clear, unsupported selectors and filter-region resource admission.
- The first expanded Renderer Release run passed 2,800 tests and failed three, with three skipped. All failures identified expected recipe changes: two old catalog fingerprints and the edge-glow scope assertion that previously named Flowchart only. The fingerprint and explicit Sequence scope were updated after inspecting the compiled value and consumer output.
- Simplification completed three serial lenses in one independent Codex context with no retained suggestions. Receipt: `/tmp/sequence-message-shadow-simplify-776d2f0cf.json`.

- Final native Release regression passed **10 tests, none skipped or flagged leaky**, including 36 combinations of width 2/12, filled/cross/point markers, horizontal/curved-self/right-angle-self geometry, and effects off/on. Every marker reference is removed independently as a paint negative control. Opaque marker color retains its original majority-cyan assertion on a separately unfiltered Message render; the actual filtered render independently must change when that marker is removed.
- The first native run passed eight (one flagged leaky) and failed two old color assertions: translucent halos increased the changed-pixel denominator. The corrected test separates opaque color from actual filtered contribution; it does not lower the original color threshold. The final full recheck passed without a leak flag; the transient initial flag's cause was not established.
- Native composed-shadow tests exercise linear RGB and sRGB, Previous versus SourceGraphic chaining, actual red/blue PNG pixels, localized PDF receipts and an explicit lower filter-primitive budget. Public preset export reports six actual filters/references and ten shadow stages in PNG and PDF, with HostDependent/SystemFonts admission rather than a bundled-font guarantee.
- Chromium 151.0.7922.34 passed 19 scenes: the public fixture plus 18 glow cases covering both widths, three marker types and three geometries. Expanded transparent-viewBox raster checks found zero painted pixels outside production bounds (alpha threshold 32, one-pixel boundary allowance). Removing actual Message filter references changed more than 20 pixels in every scene. This is visible-effect and clipping evidence, not perceptual equivalence to the entire reference theme.
- Fresh native SVG/PNG/PDF outputs, browser screenshots and a PDFium 96-dpi raster were generated and inspected. PDFium is an existing host library, not a project dependency. Captures, source/rlib hashes, browser script and PDFium identity are under `target/bench/experiments/sequence-message-shadow-776d2f0cf/`.
- Formal source review completed seven serial lenses in one independent Codex context with no retained findings. Receipt: `/tmp/compound-engineering-501/ce-code-review/20260919-101753-790aaa94/review.json`. Its completed `native-marker-helper-addendum.json` confirms that the later marker-test separation preserves the real filtered negative control and every original opaque-color threshold.

Final Renderer Release regression passed **2,803 tests, three skipped**: library, Sequence SVG and theme-resolution integration targets. This includes the corrected catalog fingerprint/scope checks and unchanged source-weight regressions. The complete 35-group default SVG sweep passed with `--check-dom --dom-modes structure,parity,parity-root --dom-decimals 3 --diagnostic-browser-text-layout --report-root`. Sequence rendered 320 of 322 fixtures (two existing skips), with 960 DOM comparisons and the same 12 exact browser-text residual comparisons; no baseline or residual receipt was changed.

Scoped Release Clippy for Renderer and the facade passed, with 160/31 warnings, matching the preceding counts. Workspace formatting and `git diff --check` passed. This is not a warning cleanup, an installed-package rebuild, or a full workspace/platform acceptance run. Parity/lint logs: `/tmp/sequence-message-shadow-{parity,clippy,fmt-final}.log`. Logs: `/tmp/sequence-message-shadow-{red,sequence,renderer,renderer-final,native,native-final}.log`.

## Remaining scope

This increment does not complete the Note, MessageLabel/ActorLabel text, Lifeline or Loop effect surfaces. It does not certify the complete reference scene, rebuild installed consumers, close the transport/profile matrix, or measure the branch's complete performance and artifact-size gates. Existing host-font dependence remains visible. No budget is relaxed.
