# XY Chart text glow consumer

Date: 2026-09-19 (local). Source baseline: `508a9458c` plus this increment.
Plan: [theme product boundaries](../../../plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md), U8.
Status: bounded consumer increment; public recipe and complete scene qualification remain open.

## Consumer boundary

XY Title, AxisTitle and Legend consume static effect bindings and winning static rules through
existing shadow lowering. Generic Text, Axis and AxisLabel effects, ordinal role effects and
unsupported graph primitives retain residual evidence. Clear suppresses the binding; absent
roles remain NotApplicable. Unsupported sibling facets cannot certify an otherwise consumed rule.

Public support discovery reports these three effect facets as TypedPartial; the unpublished
manifest revision remains 1. The existing paint accounting owns each whole rule. Terminal observations bind text, paint,
font size/weight, filter reference, transform, anchor and baseline to actual writer output.
Text and series use one family-owned shadow recorder and expected application count. No new
receipt protocol, public API, dependency or embedded font is introduced. The no-effect path
skips effect allocation and text-effect placement observations.

## Layout and paint allocation

Effected and control layouts use the same selected TextMeasurer. The effect does not switch
measurement profiles, shape text separately or alter label layout. The writer allocates local
user-space filter regions and transforms their four corners into root paint bounds. Absolute
units preserve blur margins when the SVG host resolves a different text metrics rectangle.

The first real PNG/PDF test correctly failed native containment: a title measured as 86.8px
had actual outline width about 89.1px. Its four-sigma shadow extended beyond the local filter;
the root viewport already had enough room. The writer now reserves one current font em on
each side of the measured rectangle before applying the shared shadow envelope. This is a
bounded paint allocation policy, **not** an actual ink bound or a font guarantee. Long strings
or unusual host metrics can exceed it. The exporter independently checks resolved glyph ink
and still rejects incomplete containment. System/host font admission remains host-dependent.

A zero-width measurement keeps the requested effect unverified; BestEffort can return the
unfiltered chart, while strict admission rejects it. Unsupported lowering is explicitly
unsupported rather than an incomplete writer obligation. Raw themeCSS retains its existing
evidence invalidation boundary.

## Verification

- Initial strict SVG test failed as expected before the consumer existed (0/1).
- Initial native positive test failed on the local region defect above; no exporter tolerance
  or containment assertion was relaxed.
- Fresh Release native probes without embedded-fonts: 2/2 passed. Both orientations export
  actual PNG/PDF with complete filter receipts and visible cyan PNG pixels. A separate deliberately
  underestimated host measurer and long title still produce NativeFilterReceiptMismatch for
  both targets.
- The first broad renderer run passed 527 tests, then the public/private support reconciliation
  test failed: discovery still described the new role effects as Unsupported. The three existing
  TypedPartial claim rows now include effect; unrelated targets and selectors remain unchanged.
- Final renderer Release library plus XY SVG integration: 2,645 passed, two skipped, using
  `--no-default-features --features layout-cytoscape`. The manifest reconciliation and new
  writer mutation, ownership, Clear, unsupported and measurement regressions passed.
- Final facade Release regression: 13/13 passed across `theme_xychart_text`,
  `theme_xychart_series` and `theme_composed_effects`, using
  `--no-default-features --features svg,png,pdf,layout-cytoscape`. This includes public recipe
  exchange and actual PNG/PDF output, plus existing State/Flowchart/XY composed shadows.
- `cargo fmt --all -- --check` and `git diff --check` passed.
- `cargo clippy -p merman-render --no-default-features --features layout-cytoscape --lib`
  succeeded with 170 warnings. Most concern existing unused code or helpers; this increment
  adds an eight-argument warning on `PaintAccounting::observe` after explicitly passing effect
  consumption alongside paint and typography. This is not a warning-free result. Retaining one
  whole-rule accounting call avoids splitting evidence ownership; no extra generic observation
  framework is introduced merely to satisfy the argument-count heuristic.

The final commands used shared `target`, `CARGO_BUILD_JOBS=1`, and Release nextest for tests.
This is scoped verification, not a full workspace/browser/platform build or a benchmark.

## Independent review

The `ce-simplify-code` reuse, quality and efficiency lenses removed duplicate attribute lookup
and passed the existing family artifact directly to its writer. One independent Codex context
performed these lenses serially; this was not three independent reviewers.

The actual `ce-code-review mode:agent` run `20260918-192329-ab4f4ce8` completed against the ten
Rust files above baseline `508a9458c`, source-set SHA-256
`ed86ef1cfb1a683c0667fd95ffe59e8b2052ac4d79c403ee69a56f89932b8e57`.
One independent context covered correctness, testing, maintainability, API contract, adversarial,
project standards, security, reliability and performance serially because thread capacity was
limited. The first review missed the public support claim update; the actual full regression found it.
The reviewer subsequently checked that correction and the three scope documents, preserving
the initial receipt. No actionable findings remained. Runtime checks belong to the parent; the receipt
requires final regression completion and does not establish full scene or release qualification.
The justified unmeasured filter-area cost remains assigned to U10; it does not prevent this
bounded consumer from landing. No tracker ticket or external publication was created.

## Remaining scope

This increment does not change the public Cyberpunk recipe, promote qualification cells, close
U8 or freeze C7a. Reference role-specific alpha/sigma, multilingual complete scenes, actual
browser geometry and public SVG/PNG/PDF scene acceptance remain required. The extra filter
surface allocation has not been benchmarked; U10 must measure its cost before any performance
or size claim. Existing terminal string duplication is unchanged and is not an optimization claim.
