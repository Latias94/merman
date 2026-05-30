# Flowchart Text Style Parity TODO

## TSP-010 Shared Mermaid label-style classification

- Status: done
- Owner: Codex
- Scope: route the Mermaid label-style key set through shared style helpers and flowchart compiled
  styles.
- Validation: focused SVG test proving `font-style`, `text-decoration`, and other label-only keys
  land on label styles instead of node shape styles.
- Handoff: unlocks later diagram-wide consolidation with treemap/state label-style lists.

## TSP-020 Relative font-size measurement

- Status: done
- Owner: Codex
- Scope: make flowchart layout resolve `font-size` values expressed as `%`, `em`, and `rem` against
  the current inherited text style.
- Validation: focused layout test proving a `font-size:50%` class changes node label dimensions.
- Handoff: keep `calc(...)` and browser keyword sizes explicit follow-ups unless fixture evidence
  requires them.

## TSP-030 Whole-label font-style measurement

- Status: done
- Owner: Codex
- Scope: carry `font-style` through layout measurement and apply Mermaid-derived italic width
  deltas for whole-label CSS styles.
- Validation: fixture-derived test with `font-style:italic` on a flowchart node label.
- Handoff: implemented as a flowchart-specific metrics input instead of widening `TextStyle`;
  revisit the global type only when another diagram has matching fixture evidence.

## TSP-040 Spacing and line-height measurement semantics

- Status: pending
- Owner: unassigned
- Scope: model browser effects for `letter-spacing`, `word-spacing`, and any label-level
  `line-height` behavior that survives Mermaid's div style overrides.
- Validation: upstream fixture or generated probe with exact `foreignObject` width/height evidence.

## TSP-050 Cross-diagram cleanup

- Status: pending
- Owner: unassigned
- Scope: remove duplicate label-style allowlists in diagram-specific renderers once flowchart is
  stable.
- Validation: diagram-specific SVG tests for treemap/state/block/ER where applicable.
