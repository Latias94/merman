---
type: Verification
title: Embedded theme font retirement
timestamp: 2026-09-21
base_commit: dfe54f279
---

# Scope

The local `preserve/embedded-fonts-theme` branch retains the complete implementation at
`dfe54f279`. The `refactor/presentation-theme-model` working tree removes the optional
`embedded-fonts` capability, font parsing/decompression, native theme shaping, SVG font
injection, and the dedicated portable-font acceptance/fuzz machinery. Nothing was published.

Themes continue to select font-family names, sizes, weights and spacing. Host measurement,
ordinary theme presets, canvas/effects, and system-font PNG/PDF export remain available.
Resource-bearing theme recipes remain readable for exchange, but compilation rejects them
explicitly. Bindings return InvalidArgument with generic error kind and no missing-capability ID.
The removed NativeCatalog fallback is no longer advertised as a usable fallback.

Wire resource types and shared host/export text contracts remain. This change does not claim
that every font-related type was removed. Existing export/math backends can still depend on
rustybuzz, ttf-parser, and decompression libraries; removing renderer dependencies does not imply
that those packages disappear from the entire workspace lockfile. No matched binary-size or
latency measurement was made, so source-line reductions are not artifact-size evidence.

# Verification

- Minimal bindings SVG compilation passed; 16 focused admission, font-resource rejection,
  ordinary typography and real C consumer tests passed.
- Rust renderer/facade/bindings suite: 4,622 tests run, 4,618 passed, 4 failed, 8 skipped.
- The same four failures reproduced on detached baseline `dfe54f279` with identical default
  package/features selection; see the baseline section below.
- Internal theme acceptance: all 67 tests passed, including host-dependent preset qualification
  and legacy projection retirement checks.
- All 11 representative feature-matrix builds and the 34-profile descriptor validation passed.
- Representative artifact dependency closures passed.
- Capability, binding-contract and native ABI projections passed freshness checks.
- Android Rust transport and Flutter/Dart real consumer verification passed.
- External-consumer theme acceptance boundary verification passed.
- Full release SVG structure comparison passed with `--check-dom --dom-mode structure
  --dom-decimals 3 --diagnostic-browser-text-layout`.
- Python artifact/workflow/fuzz/license contracts passed; Web descriptor tests: 10 passed.
- Generated source license reports, release projections and third-party license checks passed.
- cargo-deny advisories/bans/licenses/sources and changed-workflow actionlint passed.
- Formatting and diff whitespace checks passed.

# Baseline failures

The following failures also occur before retirement:

1. `diagram_theme_covers_additional_current_diagram_surfaces`
2. `diagram_theme_covers_core_diagram_roles`
3. `diagram_theme_series_palette_reaches_supported_ordinal_diagrams`
4. `treemap_unverified_source_fonts_fail_closed_for_portable_themes`

The first three concern Mermaid compatibility ownership versus typed preset border/palette
expectations. The fourth expects dynamic frontmatter title font size to survive even though
source config filters `var()` and falls back to a static size. Their relevant source and test
files were unchanged by this retirement. The baseline was checked out at
`/tmp/merman-font-retirement-baseline` and shared the existing target cache; no branch pointer
or existing source was reset.

# Local evidence

Logs are retained under `/tmp/merman-retire-*`, including `tests-all.log`,
`baseline-tests.log`, `acceptance-tests.log`, `feature-matrix.log`, `closures.log`,
`platforms.log`, `acceptance-boundary.log`, `svg-structure.log`, and `licenses-final.log`.
These are local verification records, not release artifacts. Full all-platform packaging,
matched performance/size measurements, and the four baseline theme failures remain outside
this retirement's verification claim.
