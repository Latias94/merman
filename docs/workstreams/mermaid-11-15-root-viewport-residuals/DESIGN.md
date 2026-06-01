# Mermaid 11.15 Root Viewport Residuals

Status: Active
Last updated: 2026-06-01

## Why This Lane Exists

The Mermaid 11.15 complete-adaptation campaign has made the implemented diagram matrix green in
structural SVG DOM `parity` mode. The remaining full-gate red surface is `parity-root`: root
`viewBox` and `style="max-width: ..."` differences.

Those differences mix several mechanisms:

- source-derived layout/root rules that can still be implemented,
- browser `svg.getBBox()` and `getComputedTextLength()` lattice behavior,
- font fallback and text measurement drift,
- retained root pins that may be stale after 11.15 refreshes,
- and accepted diagnostic residuals that should not block structural adaptation.

This lane keeps that work separate from the 11.15 adaptation closeout so the project can state the
implemented 11.15 matrix accurately without pretending browser-root exactness is fully solved.

## Target State

This lane closes when one of these is true for each residual bucket:

- a deterministic Mermaid-source-derived rule replaces the residual;
- a generated, version-pinned browser metric table explains the residual and passes no-growth
  governance;
- a root override is proven stale and removed/refreshed;
- or the residual is explicitly accepted as diagnostic browser/font/root lattice drift with a
  narrow policy entry and fresh report evidence.

The full `parity` gate must remain green throughout.

## In Scope

- Classify fresh `target/compare/*_report_parity_root.md` residuals by diagram and mechanism.
- Fix source-derived root/layout rules where Mermaid 11.15 source explains the drift.
- Improve `xtask compare-all-svgs --dom-mode parity-root` so it produces bounded, useful failure
  summaries.
- Update root residual policy entries only with fresh evidence.
- Track override footprint using `cargo run -p xtask -- report-overrides --check-no-growth`.

## Out Of Scope

- Reopening structural Mermaid 11.15 SVG DOM parity, unless a root investigation exposes a real
  structural regression.
- Adding hand-written per-string text constants at renderer call sites.
- Introducing a runtime browser dependency into normal rendering.
- Implementing deferred upstream diagram families.

## Architecture Direction

Separate measurement surfaces explicitly:

- HTML foreignObject content box,
- SVG `getComputedTextLength()` advance,
- SVG `<text>.getBBox()` extents,
- final root `svg.getBBox()` plus Mermaid padding/serialization.

Work should move constants out of renderer call sites and into either deterministic layout logic or
generated/version-pinned measurement tables. If a row cannot be made source-derived or generated,
document it as diagnostic residual rather than silently widening tolerances.

## Closeout Condition

- Structural `compare-all-svgs --dom-mode parity` is green.
- `parity-root` is green or fails only with accepted, documented diagnostic residuals.
- Override growth checks pass.
- Any remaining larger source-rule buckets have child tasks or workstreams.
