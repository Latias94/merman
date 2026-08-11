# ADR-0062: No Production Fixture Overrides

## Status

Accepted

## Updated

2026-08-11 for Mermaid `@11.16.1`

## Context

Merman is a headless implementation of a pinned Mermaid release. Official SVG baselines are
produced by a browser and therefore contain platform-dependent results from font fallback,
shaping, hinting, `getBBox()`, `getComputedTextLength()`, and SVG serialization.

The previous architecture copied some of those results into production tables keyed by fixture id
or complete label text. That made a known corpus reproducible, but it also let fixtures become a
second implementation of rendering semantics. A new input with the same grammar and style could
take a different path merely because its full text was absent from a generated table.

## Decision

Production rendering must not use fixture ids or complete source/label strings as lookup keys.
Fixture baselines are verification inputs, never runtime inputs.

### Root viewports are computed

Every family supplies source-backed content bounds and a root algorithm to the shared Root Viewport
module. Root Viewport normalizes finite dimensions, applies padding and sizing rules, emits root
attributes, and finalizes deferred documents from emitted content bounds.

There is no generated root table, root-override policy, audit mode, or fixture-id lookup. A root
parity difference must be fixed in semantics, layout, emitted geometry, measurement, or the
family's root algorithm. A browser-only residual may remain documented in verification evidence;
it must not be copied into production as a pin.

### Text measurement uses general facts

Text measurement follows the operation-owned phase route defined by ADR-0057:

1. A successful host measurement is authoritative for that operation and bypasses vendored facts.
2. The deterministic headless fallback may use generated facts keyed by properties that generalize
   to unseen text: DOM shape, font stack, font size and weight, glyph advances, kerning pairs,
   trigrams, and endpoint overhangs.
3. Generated measurement facts must come from synthetic browser probes or another reproducible font
   measurement source. Fixture text may select validation coverage, but must not train a value keyed
   by the fixture or the complete string.

Full-string HTML widths, SVG extents, family label widths, and Sequence SVG tables are forbidden.
Family-owned constants remain valid only when they are direct projections of an upstream algorithm
or configuration default and apply independently of fixture identity and label text.

### Verification owns residuals

Comparator normalization remains narrow and non-semantic. Accepted browser residuals are explicit
verification policy describing a bounded mismatch set; they do not alter production output.

Architecture and generation gates enforce the boundary:

- production Rust sources contain no fixture-id or complete-text override modules or symbols;
- generated measurement facts contain no complete fixture strings;
- browser-probe generators are deterministic and validate against an independent fixture corpus;
- host-measurement tests prove that a successful host result bypasses the vendored fallback; and
- structural and normal parity continue to reject new or changed mismatches. Root parity uses the
  blocking root viewport contract and deterministic exact fixture set described by ADR-0050;
  browser-owned bbox numerics are emitted only as attributable diagnostics and never as production
  fixture data.

## Consequences

- Production behavior generalizes to unseen diagrams instead of recognizing the fixture corpus.
- Root and text parity failures expose the owning semantic, geometry, or measurement problem.
- Browser-dependent results cannot always be reproduced exactly by the deterministic fallback.
  Hosts that require their system-font geometry must install a host measurer.
- Generated data remains appropriate for reproducible font facts, but not for fixture answers.
- The former `report-overrides`, `audit-root-overrides`, root policy, generated root tables, and exact
  text generators are removed rather than retained as migration paths.

## Rejected Alternatives

### Keep a bounded override budget

Rejected. A non-growing table is still a second behavior path and still fails for unseen text.

### Hide residuals in comparator normalization

Rejected. Broad normalization can erase semantic or geometry regressions and provides no runtime
benefit to users.

### Implement a complete browser in Rust

Rejected. The CSS, font fallback, shaping, and SVG text-layout surface is not a bounded renderer
dependency. Host measurement is the correct authority when exact system-font behavior matters.

## Related Decisions

- ADR-0014: Upstream Parity Policy
- ADR-0049: Vendored Font Metrics for Headless Parity
- ADR-0050: Release Quality Gates
- ADR-0057: Headless SVG Text `getBBox()` Approximation
- ADR-0073: Family-Owned Diagram Architecture
