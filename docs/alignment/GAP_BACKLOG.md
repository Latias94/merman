# Gap Backlog (Mermaid@11.15.0)

Baseline: Mermaid `@11.15.0` (see `tools/upstreams/REPOS.lock.json`).

This document tracks **known gaps vs “perfect” Mermaid parity** and a plan to systematically
eliminate them without regressing the global parity gates.

Scope:

- Primary contract: SVG DOM parity in `parity` mode (DOM structure/semantics; geometry numbers normalized).
- Secondary contract (tracked, non-blocking in CI today): SVG root viewport parity in `parity-root` mode
  (root `max-width`/`viewBox` lattice, `--dom-decimals 3`).
- Secondary contracts:
  - strict SVG XML parity where feasible (`dom-mode strict`)
  - deterministic, reproducible upstream baselines
  - headless-first library ergonomics

Global gates (must stay green):

- `cargo nextest run`
- `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`

Non-blocking CI signal (kept visible to drive incremental alignment work):

- `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`

The global `parity-root` sweep skips Phase 2 Stage B families whose current family-local audits
still report root viewport residuals: `treeView`, `ishikawa`, and `eventmodeling`. Use
`compare-all-svgs --diagram <family> --dom-mode parity-root` or the family-local compare command
when auditing those root residuals explicitly.

## Automated audits

This repo contains a lightweight “gap audit” command to keep parity work driven by repeatable data instead of ad-hoc
spot checks:

- Generate a report: `cargo run -p xtask -- audit-gaps --out target/audit/gaps.md`
- Output is intentionally written under `target/` (do not commit it); only summarize conclusions here.

As of `2026-06-22` (see the generated report for details):

- Parser-only fixtures: `6` (not included in SVG DOM parity gates)
- Deferred fixtures (`fixtures/_deferred`): `0` parse OK, `86` parse ERR, `2` absorbed duplicates
- The absorbed duplicates are Mermaid-reachable ELK requests that now have active source-backed evidence:
  the Class ELK Cypress full-diagram body is covered by the active Class fixture, and the Flowchart docs
  `layout: elk` example is covered by `fixtures/flowchart/upstream_docs_layouts_how_to_use_001.mmd`.

Notes:

- `xtask audit-gaps --check-upstream-render` highlights “actionable gaps”: parser-only fixtures that upstream Mermaid
  CLI can render successfully.
- `xtask audit-gaps --check-upstream-render-deferred-ok` checks which deferred-but-parseable fixtures upstream CLI can
  render, and lists “promotable candidates” (in-scope + upstream renders OK) to guide incremental fixture promotion.

## Gap Index

Per-diagram details:

- Architecture: `docs/alignment/ARCHITECTURE_SVG_PARITY_GAPS.md`
- Flowchart: `docs/alignment/FLOWCHART_SVG_PARITY_GAPS.md`
- Mindmap: `docs/alignment/MINDMAP_SVG_PARITY_GAPS.md`
- State root viewport: `docs/alignment/STATE_ROOT_VIEWBOX_PARITY_GAPS.md`
- Flowchart root viewport: `docs/alignment/FLOWCHART_ROOT_VIEWBOX_PARITY_GAPS.md`
- Flowchart strict XML: `docs/alignment/FLOWCHART_SVG_STRICT_XML_GAPS.md`

## Backlog (prioritized)

Legend:

- Priority: P0 (must), P1 (should), P2 (nice)
- Risk: L/M/H (probability of regressions / breadth of impact)

### P0: Parity debt (must eliminate)

1. **Remove fixture-scoped renderer special-cases**
   - Target: no diagram-specific code paths keyed by fixture id; replace with topology/semantics-driven rules or
     fully align measurement + edge routing so upstream parity emerges naturally.
   - Current status: Architecture Stage B is now free of fixture-id keyed wrapping / formatting adjustments; keep it
     that way as we tighten geometry-level fidelity.
   - Risk: M (wrap/measurement and geometry changes can ripple through many fixtures).

2. **Converge headless layout/measurement so wrap decisions match upstream**
   - In practice: align “effective” `createText()` width and `getComputedTextLength()` behavior
     for SVG labels under the pinned CLI baseline.
   - Risk: H (can change line breaks, bboxes, viewBox/max-width).

3. **Reduce fixture-id keyed root viewport overrides**
   - Target: replace fixture-id keys with reusable semantic/topology profiles or deterministic
     algorithms wherever possible.
   - Risk: M (viewport is a global gate and sensitive to tiny drift).

### P1: Coverage confidence (expand and stabilize)

4. **Increase fixtures for sensitive diagrams (10–30 per batch)**
   - Flowchart, State, Sequence, Architecture, Class, Mindmap.
   - Sources: `repo-ref/mermaid/docs/syntax/*.md` + targeted issue repros + stress fixtures.
   - Risk: L (mostly additive), but may surface real parity bugs.

5. **Clarify and document “CLI vs browser parser” mismatches**
   - Architecture shorthand forms appear in upstream Cypress but Mermaid CLI renders them as error.
   - Policy: Mermaid CLI output is the baseline for upstream SVG baselines; browser-only behavior
     stays snapshot-gated or normalized.
   - Risk: L (documentation + fixture policy).

6. **Reduce “parser-only” fixtures by implementing missing semantics**
   - Remaining candidates (from `xtask audit-gaps --check-upstream-render`) should now be re-audited after the
     Flowchart KaTeX promotion; the previous Flowchart `$$...$$` math-label backlog item is no longer parser-only.
   - Risk: M (can touch parsing + rendering + DOM parity).

### P2: Beyond core parity (optional expansions)

6. **Family-specific `look=handDrawn` parity**
   - Flowchart, Class, ER, Requirement, and State have focused rendered seed evidence. Venn and
     Ishikawa RoughJS branches remain deferred family lanes and must be promoted only with
     source-backed rendered tests.
   - Risk: H (shape/path output diverges substantially).

7. **Mermaid-reachable ELK hardening**
   - Source-backed Flowchart ELK is the default public render path and the dedicated probe gate
     covers every exact upstream `flowchart-elk.spec.js` render call and all 57 unique layout
     bodies; broad SVG parity admits those probes under the source-backed backend. Class
     `layout: elk` and `class.defaultRenderer: elk` now dispatch through the Class ELK adapter
     under the existing `elk-layout` feature and reuse the Class SVG renderer. Remaining work is
     continued hardening from new upstream or user-reported cases, not a generic deferred
     `layout=elk` gap.
   - Risk: M/H (large surface area, but the current Mermaid spec body coverage is source-backed).

8. **ZenUML “practical” compatibility expansion**
   - Snapshot-gated only (no upstream SVG baselines).
   - Risk: M (translator complexity; must not regress Mermaid parity gates).

## Milestone mapping

This backlog is executed via release-oriented milestones in:

- `docs/alignment/MILESTONES.md`
