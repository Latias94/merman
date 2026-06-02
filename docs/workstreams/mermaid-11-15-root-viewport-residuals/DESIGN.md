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

## Residual Taxonomy Mapping

This lane adopts the headless-parity-deepening taxonomy and applies it to the current active front:

- **Source-backed behavior gap**
  - Sequence actor-type geometry and footer-row root-bounds work already closed rows here.
  - Class namespace extraction/order and title-wrap source alignment already closed rows here.
  - Architecture junction parent assignment, group alignment overwrite order, and group padding
    defects were source-backed rows in this class.

- **Generated measurement gap**
  - Sequence wrap/HTML `<br>` work moved through generated SVG text evidence rather than ad hoc
    constants.
  - Class long `htmlLabels=true` title widths currently point here if a generated replacement path
    can replace stale hand-curated lookup rows.
  - Future Architecture Cytoscape canvas-label evidence may also land here if a reusable generated
    path proves better than the current deterministic approximation.

- **Browser lattice tail**
  - Flowchart's current 61-row bucket is mostly small root-width/viewBox tails after the major
    11.15 source-rule fixes.
  - Timeline's remaining 3 rows and Journey's remaining 2 rows are in this class today.
  - Sequence still has note/wrap/participant tails that are likely here unless stronger source or
    generated evidence changes the classification.

- **Stale baseline or stale override**
  - C4, ER, Sankey, and several Sequence/Flowchart/Architecture rows were closed by refreshing or
    deleting stale root pins or stale upstream baselines.
  - Architecture `service_icon_text` and the old `reasonable_height` calibration are canonical
    examples of this class.

- **Solver / phase residual**
  - Architecture is the main active owner of this class: disconnected components, compound bounds,
    and some group/port rows now sit here after source inputs were matched.
  - Rows such as `stress_architecture_group_port_edges_017` and
    `stress_architecture_disconnected_islands_046` are currently classified here.

- **Scope boundary**
  - Unsupported Mermaid families remain outside this lane.
  - `flowchart-elk` remains a deferred capability decision, not a residual to “smooth over”.

Current bucket summary:

- Flowchart `61`: mostly browser lattice tails, with occasional generated-measurement follow-up
  candidates.
- Architecture `29`: mixed source-backed rows already reduced; remaining front is mainly
  solver/phase residuals plus some browser/generation measurement tails.
- Sequence `27`: mixed generated-measurement gaps and browser lattice tails.
- Class `12`: generated-measurement gap plus stale-table audit front; do not solve by table growth.
- Timeline `3`, Journey `2`: browser lattice tails unless stronger evidence emerges.

## Closeout Condition

- Structural `compare-all-svgs --dom-mode parity` is green.
- `parity-root` is green or fails only with accepted, documented diagnostic residuals.
- Override growth checks pass.
- Any remaining larger source-rule buckets have child tasks or workstreams.
