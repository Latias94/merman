# Flowchart host-measured text shadows

Date: 2026-09-19. Parent: `1bb2dc253`. Status: bounded increment verified; U6 and C7a remain open.
This bounded increment does not close U6, public preset qualification or C7a.

## Consumer boundary

Ordinary native SVG NodeLabel and EdgeLabel shadows reuse the existing prepared host
measurement and wrapped rows. They no longer require caller-provided font assets. The
one-em reserve allocates filter space; it does not certify arbitrary host-font ink.
The existing native-font path retains its shaped ink bounds. Source/style identity,
structural CSS guards, actual writer placement and native final-target checks remain
mandatory. Custom measurers without a prepared operation binding retain residual evidence.
There is no new font, dependency, public type, protocol version or retained label cache.

The native filter IDs now contain the existing `-theme-effect-` marker. Their former
`-theme-label-effect` spelling was not recognized by the native export observer: visible
pixels alone did not prevent `NativeFilterReceiptMismatch`. Tests now check that reason,
filter counts, PNG pixels and PDF localization, for host and supplied-font paths.

## Explicit line break correction

The stronger native fixture exposed an independent shared preparation error. With a finite
wrap width, `validate_prepared_text_coverage` rejected every LF between adjacent prepared
lines. Both themed and no-effect controls failed with `InvalidBackendEvidence`; diagnostics
located the failure in preparation, before Flowchart source reprojection.

An adjacent pair can now cross one explicit LF, with the existing whitespace policy on
both sides. Leading/trailing LF still needs its own line record, and two LF cannot skip an
empty line. Lines cannot consume LF as text. Tests cover SVG/HTML wrapping, normal/pre-line/
pre-wrap spacing, preserved blank rows, missing rows and non-breaking spaces. No backend
attestation, source-range or glyph-coverage validation is removed.

## Reference observation

A read-only browser study used pinned Modern Mermaid `a021cbce`, its installed Mermaid
11.12.1 and the playground's installed 11.17.2, with HTTP(S) requests blocked. The public
Flowchart scene has four node labels and two nonempty edge labels with one cyan text shadow
(10px CSS blur, alpha .5); default HTML leaves use weight 600. Native innermost tspans use
400 despite outer 600. The cluster title has neither glow nor stronger weight. Shape filters
are on siblings, and the edge label background must not be included in glyph filtering.

The existing sRGB sigma-5/alpha-.5 graph can serve the text contract; a new graph or CSS
interpreter is unnecessary. These are reference observations, not fresh Merman browser or
all-target qualification. Sources, captures and negative controls:
`/tmp/flowchart-public-text-reference-20260919/report.md`.

## Verification record

- Initial host-only positive test failed with `UnverifiedFamilyTheme` before implementation:
  `/tmp/flowchart-host-label-red.log`.
- First no-font renderer increment: 27/27 passed. This preceded the native ID and shared
  line-break corrections and is not the final source's full verification.
- Native first run: 1/3 passed; normal labels exposed the ID mismatch. A custom measurement
  probe could not enter the prepared geometry path, so its final oracle correctly checks
  residual evidence instead of claiming native ink-overflow coverage.
- After ID correction, native-3 controls localized the multiline failure independently of
  effects. Diagnostic output confirms `prepare "Alpha\nGyp": InvalidPreparedText`:
  `/tmp/flowchart-host-label-native-diagnostic.log`. Temporary diagnostic code was removed.
- Final facade native Release suite: **3/3 passed**. The text test runs six cases (host and
  supplied fonts, simple/nested-multiline/nested-blank-line); each validates PNG and PDF.
  `/tmp/flowchart-host-label-native-final.log`.
- First combined renderer/export Release run: **2,802/2,803 passed**, two skips. All six
  existing text-shadow native containment tests passed. The sole failure was the new sidecar
  test's bare `DeterministicTextMeasurer`, which intentionally has no operation binding.
  Its fixture now uses the actual deterministic environment session's routed measurer;
  the host/native positive and stale-source/style negatives remain unchanged.
  `/tmp/flowchart-host-label-libraries.log`.
- Final combined renderer/export Release run: **2,803/2,803 passed**, two skips.
  Includes the exact-source/style sidecar test, explicit-break admission table, six existing
  glyph/filter/root-containment negatives and all selected Flowchart effect tests.
  `/tmp/flowchart-host-label-libraries-final.log`.
- With `embedded-fonts` disabled, the Flowchart SVG and facade PNG/PDF suites passed
  **33/33**, zero skips: `/tmp/flowchart-host-label-no-fonts.log`.
- `cargo fmt --all -- --check`, `git diff --check` and scoped Clippy passed. Clippy still
  reports feature-dependent dead code and existing large-error warnings; this is not a
  zero-warning claim. `/tmp/flowchart-host-label-clippy.log`.
- Parent final verification reconciles the completed review with these runtime results in
  `parent-final-followup.json` under the same review directory. No retained review findings
  remain for this bounded increment; this is not a branch-wide release approval.
- Simplification used three full rubrics serially in one independent Codex context, with
  no suggested changes. Formal review used seven lenses in that context and retained no
  findings. Original and final-source records are separate; their then-pending runtime
  verdicts are retained: `/tmp/compound-engineering-501/ce-code-review/20260919-161158-eca274da/`.
  Source-bound final delta: `final-addendum/`; simplification:
  `/tmp/flowchart-host-label-simplify-final-addendum.json`. The test-only routed-measurer
  correction has a separate `routed-measurer-fixture-addendum.json`; source set
  `21c1aeea6c8e924184db9a42a8119a0e3899d4afadc3eaefbcab45cf6a97210d`.
  Original failed runs and review verdicts are not overwritten.

Verification commands use the shared target directory and `CARGO_BUILD_JOBS=1`:

```text
cargo nextest run --release --locked -p merman --no-default-features --features svg,png,pdf,layout-cytoscape,embedded-fonts --test theme_flowchart_edge_effects --test-threads 1 --no-fail-fast
cargo nextest run --release --locked -p merman-render -p merman-export --no-default-features --features merman-render/layout-cytoscape,merman-render/embedded-fonts,merman-export/png,merman-export/pdf,merman-export/embedded-fonts --lib --test flowchart_node_effects --test-threads 2 --no-fail-fast
cargo nextest run --release --locked -p merman -p merman-render --no-default-features --features merman/svg,merman/png,merman/pdf,merman/layout-cytoscape --test flowchart_node_effects --test theme_flowchart_edge_effects --test-threads 1 --no-fail-fast
cargo clippy --locked -p merman -p merman-render -p merman-export --no-default-features --features merman/svg,merman/png,merman/pdf,merman/layout-cytoscape,merman/embedded-fonts --lib --test flowchart_node_effects --test theme_flowchart_edge_effects
cargo fmt --all -- --check
```

## Remaining scope

Default HTML, Markdown and specialized label placements still retain their existing limits.
The public Cyberpunk Flowchart recipe has not gained text-glow bindings in this increment.
The next consumer must apply glyph shadows without filtering the edge-label background and
must keep actual target admission truthful. Full public Flowchart SVG/PNG/PDF scene
acceptance, installed transport qualification and C7a remain open.
