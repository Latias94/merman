# Sequence lifeline and activation consumers

Date: 2026-09-19. Parent: `0a9d46c72`. Status: bounded increment verified; full U7 and C7a remain open.
This U7 increment does not close the full Sequence scene, qualification or C7a.

## Drawing and product boundary

The [reference observation](2026-09-19-sequence-text-reference.md) records a cyan 2px lifeline
with a sigma-6, alpha-.6 shadow, and a translucent cyan activation with a 3px border. The public
Cyberpunk recipe adds those rules through the same scoped authoring path as custom themes.
It reuses the existing message/edge shadow graph. No dependency, font, protocol version,
qualified cell or native conversion budget is added.

Lifeline effects use actual terminal coordinates and the effective stylesheet width, including
actor-man placements. The writer produces definitions and records successful line emission;
there is no second layout map. Final root bounds include the outward-rounded filter region.

Activation geometry stays in its existing activation stack/rectangle plan. Width, radius and
the existing bounded shadow composition are consumed by the real rectangle writer, using the
same static rectangle resolution and evidence as Note. Custom activation shadows are a mechanism
regression; the public Cyberpunk recipe only requests its reference border width. Missing or
unsupported terminals remain incomplete, and source paint ownership does not suppress an
independent effect or geometry facet.

## Verification

- Before implementation, the new activation nested-rectangle test failed with
  `IncompleteFamilyTheme`; `/tmp/sequence-remaining-red.log`.
- After activation implementation, its nested geometry/effect/Clear regression passed.
  The first lifeline test attempt had an invalid empty rule; corrected before claiming red evidence.
- The valid binding-only lifeline test then failed with `IncompleteFamilyTheme` before the
  lifeline implementation; `/tmp/sequence-lifeline-red.log`.
- Initial broad renderer run: **2,850/2,854 passed**, three skips. All new behavior tests
  passed. Four failures were the exact Cyberpunk fingerprint (two checks), its new Lifeline
  graph scope and the expected public filter count. Metadata and assertions were updated from
  the compiled recipe and actual terminal count. Final Release renderer library, Sequence SVG,
  shared theme-resolution and support-discovery run: **2,908/2,908 passed**, three skips.
  Log: `/tmp/sequence-lines-rects-renderer-final.log`.
- Native composed-effect Release suite: **18/18 passed**, serially. Initial run was 17/18:
  the generic Lifeline fixture's default 0.5px stroke produced no red pixels above the existing
  threshold. The fixture now authors the public recipe's 2px width for both composition inputs;
  red/blue thresholds and the SourceGraphic reset-red-zero assertion are unchanged. This does
  not certify two-color visibility at the default 0.5px width. Logs:
  `/tmp/sequence-lines-rects-native.log`, `/tmp/sequence-lines-rects-native-final.log`.
- No-embedded-font Release checks: **5/5 passed**, 98 tests outside the selected filter.
  Log: `/tmp/sequence-lines-rects-no-font.log`.
- Default Sequence parity: **322 selected, 320 rendered, two math skips**, 960 DOM comparisons.
  All 12 encountered browser text-layout comparisons matched the accepted exact residuals.
  Report: `target/compare/sequence_report_parity_root.md`; copied into the experiment directory.
- Scoped Release Clippy, workspace formatting and diff whitespace checks passed. Clippy is
  not warning-free; this increment includes an eight-argument `write_lifeline_root_open`
  warning. No warning-free or full-workspace validation claim is made.
  Commands, exit codes and timings: `/tmp/sequence-lines-remaining-checks.json`.

Experiments: `target/bench/experiments/sequence-lines-rects-0a9d46c72/`.
Simplification ran all three persona rubrics serially in one independent Codex context:
`/tmp/sequence-lines-rects-simplify-0a9d46c72.json`. Its no-effect counting finding is addressed
by storing activation shadow data in a separate, lazily allocated vector. Ordinary rectangles
retain their original per-element shape; empty shadow counting is constant-time. The Lifeline
resolver keeps the no-route fast return while recognizing binding-only requests. No measured
performance improvement is claimed here.

Current compiled Cyberpunk identity:
`3624f30335302937a4290ad50dfb1415344ee86d8c6b6edde382b6fcb086e29f`.
Expected fixed-scene filter/reference/shadow-stage counts: **16/16/24**.

Three fresh facade exports (public recipe, zero-margin 12px line/activation stress scene,
and its effect Clear variant) produced SVG/PNG/PDF. All PNG/PDF reports retain only
`HostDependent/SystemOrHostFontDependency`; no ThemeEvidenceIncomplete,
NativeFilterReceiptMismatch or PdfNativeFilterNotLocalized was reported. The capture contract
records the rebuilt native rlib SHA-256, compile command and successful execution.

Chromium **151.0.7922.34** confirmed the public line's computed 2px width, cyan color,
sigma-6/alpha-.6 flood and filter binding; the activation has a 3px cyan border without a
recipe-requested filter. The flood alpha is encoded in rgba flood-color, not a separate
flood-opacity attribute. The diagnostic initially assumed the latter and was corrected to
inspect computed flood-color/opacity.

Expanded transparent viewport checks preserved actual terminal geometry and tested visible
paint at alpha >=32 with one-pixel allowance. Public/stress/Clear samples contained
2,720 / 27,289 / 12,982 painted pixels and **zero viewport escapes**. Removing only the line
and activation filter references changed 6,312 / 26,152 / 0 visible alpha samples; no sample
touched the diagnostic capture boundary. Browser, native PNG and PDFium public views plus
the native stress PNG were inspected. PDFium library provenance and 96dpi outputs are in
`pdfium-observations.json`. These are local host observations, not full reference equivalence.

Formal review completed eight lenses serially in one independent Codex context with zero
retained findings: `/tmp/compound-engineering-501/ce-code-review/20260919-141551-d0420e95/`.
The later 2px native fixture adjustment received a narrow follow-up in
`lifeline-fixture-addendum.json`. The initial Not ready verdict was conditional on the
remaining parent-run native/browser/parity/no-font/lint checks, all now completed as recorded
above. The original review receipt is retained without rewriting its earlier verdict. No external-model review or
eight independent reviewer contexts are claimed. The reviewer also investigated a private
missing-layout mutation and found no public path to replace Sequence's immutable prepared
layout; that hypothetical mutation is not promoted into a production defect.

Installed packages and the full platform matrix have not been rebuilt for this increment.

## Control structure contract still open

The current `Loop` paint consumer is the keyword polygon, not the frame lines. The next
migration must deliberately split `Loop` (frame), `LoopLabelBackground` (polygon), and
`LoopLabel` (text), with current authors and evidence migrated together. Existing polygon
radius remains unsupported: an SVG polygon does not acquire rounded corners from CSS rx/ry.

The inventory currently has 50 semantic targets. Its 57 projection channels and 56 historical
retirement rows are different quantities. A new target must not increase those unrelated
inventories. The eight current box authorization rows move to the background target while
retaining the historical box projection bits/contribution identities; the new frame must not
claim those authorizations. Immutable KTD23 retirement records remain unchanged. The current
KTD17 authority fingerprint must be reconciled through the existing authorization process.
This is a migration boundary, not evidence that the frame implementation is complete.
