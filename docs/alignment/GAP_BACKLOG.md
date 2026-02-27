# Gap Backlog (Mermaid@11.12.3)

Baseline: Mermaid `@11.12.3` (see `tools/upstreams/REPOS.lock.json`).

This document tracks **known gaps vs “perfect” Mermaid parity** and a plan to systematically
eliminate them without regressing the global parity gates.

Scope:

- Primary contract: SVG DOM parity in `parity-root` mode (viewport + DOM structure, `--dom-decimals 3`).
- Secondary contracts:
  - strict SVG XML parity where feasible (`dom-mode strict`)
  - deterministic, reproducible upstream baselines
  - headless-first library ergonomics

Global gates (must stay green):

- `cargo nextest run`
- `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`

## Automated audits

This repo contains a lightweight “gap audit” command to keep parity work driven by repeatable data instead of ad-hoc
spot checks:

- Generate a report: `cargo run -p xtask -- audit-gaps --out target/audit/gaps.md`
- Output is intentionally written under `target/` (do not commit it); only summarize conclusions here.

As of `2026-02-27` (see the generated report for details):

- Parser-only fixtures: `18` (not included in SVG DOM parity gates)
- Deferred fixtures (`fixtures/_deferred`): `66` parse OK, `80` parse ERR
- Most “parse OK but deferred” cases are out-of-scope config signals (`look=handDrawn`, `layout=elk`) rather than
  parser correctness issues.

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
   - Candidates (from `xtask audit-gaps`): Treemap compiled styles (`classDef`/CSS), Sankey identifier edge cases
     (e.g. `proto`), XYChart title/header variants, and any Architecture Cypress fixtures that should be CLI-compatible.
   - Risk: M (can touch parsing + rendering + DOM parity).

### P2: Beyond core parity (optional expansions)

6. **Flowchart `look=handDrawn` parity**
   - Requires RoughJS-style path generation parity (or a compatible deterministic port).
   - Risk: H (shape/path output diverges substantially).

7. **Flowchart `layout=elk` parity**
   - Requires ELK layout integration with deterministic version pinning.
   - Risk: H (different layout engine, large surface area).

8. **ZenUML “practical” compatibility expansion**
   - Snapshot-gated only (no upstream SVG baselines).
   - Risk: M (translator complexity; must not regress Mermaid parity gates).

## Milestone mapping

This backlog is executed via release-oriented milestones in:

- `docs/alignment/MILESTONES.md`
