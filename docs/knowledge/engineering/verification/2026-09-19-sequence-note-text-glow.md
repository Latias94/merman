# Sequence NoteLabel effects and the public Cyberpunk recipe

Date: 2026-09-19. Base: `c72ad55b1`. Status: bounded increment verified; U7 remains open.
This is a bounded U7 increment, not complete Sequence scene or C7a qualification.

## Implementation

Static unqualified/Default NoteLabel effects and effect bindings reach the actual SVG text
terminal. Final decoded/wrapped lines, coordinates and typography come from the existing Note
writer. The shared shadow materializer uses absolute SVG coordinates and a one-em allocation
reserve. This reserve is not a guarantee about arbitrary host glyph ink: native export still
checks actual converted glyph bounds and rejects a deliberately underestimated host measurer.

Sequence now uses the existing deferred-root path only when a text effect needs final paint
bounds. The same bounded output buffer receives the finalized viewBox/max-width; it does not
retain a second body or rerun wrapping. Ordinary and Clear output retain the immediate-root
path. No new dependency, font, public schema or general drawing/proof abstraction is introduced.
Support discovery adds only the consumed NoteLabel facet. Math/foreignObject shadows and
unsupported sibling facets remain residuals; Clear is consumed without declaring a filter.
Missing terminals and receipts for the wrong text surface cannot establish completeness.

Explicit empty SVG rows and the writer's zero-width placeholder retain their line placement,
but produce no filter. Their consumption is distinct from actual filter applications. An
all-paintless binding is NotApplicable; a complete static rule records semantic consumption
without a Shadow/SvgFilter claim. This is based on known text semantics, never a zero returned
by a host measurer.

The public Cyberpunk recipe adds a Sequence-only `cyberpunk-note-text-glow`: sRGB, magenta alpha
.4, sigma 4. It retains the existing readable magenta fill and actual reference weight 400.
See [the observed reference contract](2026-09-19-sequence-text-reference.md) for the reference's
inactive 600-weight declarations and dark leaf-color override. Recipe compilation and exchange
share this specification. The fingerprint is
`6af89f7ef7149c8896053f6863beca00fb8d9b943f2b8399f3bfa97eab97b403`.
Unpublished versions stay at 1; no catalog cells were promoted.

## Verification and development findings

- The initial strict NoteLabel effect regression failed with `IncompleteFamilyTheme`, before
  production changes. The first implementation exposed an incorrect Clear filter capability;
  Clear now records semantic consumption using the existing convention.
- The first native run passed 11/12. Its mixed empty-line test exposed an actual
  `NativeFilterReceiptMismatch`; the converter removes the paintless glyph terminal. The
  renderer fix above preserves the exporter validator. The final native run passed **14/14**,
  serially, including mixed empty rows, all-empty binding, underestimated nonempty text, and the
  public preset. The public scene has 8 filters/references and 12 shadow stages.
- The first focused budget test used a parsed model without its theme parse binding and failed
  with `ThemeParseBindingMismatch`; the test now uses the same shared parse compatibility entry
  as other themed tests. Catalog fingerprints were updated from the actual compiler output.
- Chromium **151.0.7922.34** checked five fresh native-produced SVG cases: the public scene and
  source font sizes 16/48, each with and without Clear. Real computed font size, weight, leaf
  color and filter state are checked. Removing just the NoteLabel filters changes 1,505 pixels
  in the public scene; the two stress scenes change 12,041 and 56,872 pixels. Clear changes zero.
  Enlarged transparent-viewport rendering finds zero painted pixels outside the production
  bounds for all three glow scenes (alpha >=32, 1px edge allowance).
- Actual PNG/PDF exports for all five cases retain HostDependent/SystemOrHostFontDependency,
  without incomplete-theme or native-filter reasons. Public PNG, Chromium and PDFium raster
  views were inspected. PDFium used the existing local library at 96 dpi; its path and hash are
  recorded with the artifacts. This does not rebuild Web/WASM or qualify installed packages.
- Renderer library, Sequence SVG and shared theme-resolution Release regression: **2,846 passed,
  3 skipped**. The first wider run caught the old public preset filter count (7); it now expects
  8 and checks that the added sigma-4 filter belongs to the actual Note text terminal.
- No-embedded-fonts Release SVG check: **4/4 passed** (92 tests outside the selected filter;
  math support intentionally disabled). Ordinary SVG consumption does not require a bundled font.
- Full default SVG corpus: **35 groups passed** in structure/parity/parity-root modes. Sequence
  rendered 320/322 fixtures (two external-math-host skips), with 960 comparisons and the same
  12 exact browser-text-layout residual comparisons. No baseline or comparator was changed.
  Root-contract mutation/canary tests also passed **6/6** (620 outside the selected filter).
- Scoped Release Clippy completed successfully with existing warnings, not a warning-free
  result. The Note line writer already exceeded the argument-count threshold before this
  increment (9 arguments, now 10); broader dead-code and other warning cleanup is separate.
  `cargo fmt --all -- --check` and `git diff --check` passed.
- Existing CI and release-preflight explicitly include `sequence_svg_test` and
  `theme_composed_effects`; the new tests inherit those gates.

### Separate existing host-layout residual

The 48px zero-margin **Clear/no-effect** stress scene has 269 painted pixels outside its
production viewport in Chromium. A separately rendered fill-only baseline is byte-for-byte
identical to the Clear SVG, proving this is not a text-effect regression. The glow allocation
contains that scene's paint, but does not repair the general host-layout mismatch. This remains
an explicit host measurement/viewport limitation for later U7/U10 audit; no arbitrary-font or
all-size no-clipping claim is made. The fixed public scene has no observed clipping.

The isolated overflow is the right Note, `Wrap text for a different placement`, using host
Trebuchet MS. Its actual ink reaches x=1057.5 beyond the viewBox right edge x=1041; all 269
pixels cross that edge. Chromium reports a 776.45px text bbox against a 741px Note rectangle.
The inferred layout text width is 721px (rectangle minus default margins), not an instrumented
Rust measurement. The next check should capture the existing measurement profile, terminal
style and `calculateTextDimensions` result, then compare the same host-font operation. Do not
add arbitrary padding. Read-only details: `/tmp/sequence-note-host-layout-residual.json`.

An initial stress probe set `sequence.noteFontSize:48` and actually rendered 16px. Existing
Mermaid-compatible generated-root typography precedence explains that result. The final probe
uses an explicit root `fontSize:48`, checked on actual glyphs; the oracle was not weakened to
accept an unobserved size.

## Review and artifacts

The actual simplify pass used the three full rubrics in one independent Codex context. It
removed a redundant root-bound calculation. A separate issue found there restored typed
`SvgDiagramId` projection accounting instead of copying its raw string. The formal review
used seven serial lenses in one independent context, executed its findings/requirements
mechanics, and retained no source finding. It also records the native empty-line discovery and
its fix. These are not claims of seven independent reviewers or an external-model review.

- Simplify: `/tmp/sequence-note-text-simplify-c72ad55b1.json`.
- Formal review: `/tmp/compound-engineering-501/ce-code-review/20260919-115830-9a0dc56a/`.
  Its initial Not ready verdict predates the parent's final verification, which is recorded here.
  The parent completed all selected checks; the only post-review code edit updated the public
  preset test's filter count and strengthened its Note text consumer/sigma assertions.
- Logs: `/tmp/sequence-note-text-red.log`, `/tmp/sequence-note-text-focused.log`,
  `/tmp/sequence-note-text-native.log`, `/tmp/sequence-note-text-native-final.log`,
  `/tmp/sequence-note-text-renderer-final.log`, `/tmp/sequence-note-text-renderer-green.log`,
  `/tmp/sequence-note-text-no-font.log`, `/tmp/sequence-note-text-parity.log`,
  `/tmp/sequence-note-text-clippy.log`, `/tmp/sequence-note-text-root-contract.log`.
- Local artifacts: `target/bench/experiments/sequence-note-text-c72ad55b1/`: capture source and
  binary/library hashes, five SVG/PNG/PDF sets, browser probe and observations, PDFium captures,
  separately generated Clear baselines, and source/artifact SHA-256 inventory.

ActorLabel/LoopLabel glow, lifeline and remaining frame/activation semantics, complete scene
qualification, other-preset coverage and the final C7a candidate/impact audit remain open.
