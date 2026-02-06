# Parity Hardening Plan (Post 100% Baseline)

Baseline version: Mermaid `@11.12.2`.

As of 2026-02-06:

- `parity` full compare: 0 mismatch.
- `parity-root` full compare: 0 mismatch (478/478 upstream SVG baselines).

This document defines the next hardening phases after reaching baseline 100% parity for the
current fixture set.

## Goals

1. Keep global parity green (`parity` + `parity-root`) while the fixture corpus grows.
2. Reduce fixture-scoped override dependence where feasible.
3. Preserve deterministic, reproducible results for the pinned upstream version.

## Current Inventory

### Upstream SVG Corpus

- Total diagrams covered: 23
- Total upstream SVG baselines: 478

Largest fixture buckets:

- `flowchart`: 120
- `gantt`: 73
- `state`: 43
- `sequence`: 40
- `architecture`: 25

### Override Footprint (11.12.2)

Root viewport overrides:

- `architecture_root_overrides_11_12_2.rs`: 18 entries (out of 26 architecture fixtures)
- `class_root_overrides_11_12_2.rs`: 9 entries (out of 17 class fixtures)
- `mindmap_root_overrides_11_12_2.rs`: 8 entries (out of 12 mindmap fixtures)

State text/bbox overrides:

- `state_text_overrides_11_12_2.rs`: 47 `Some(...)` entries across width/height/bbox helpers

## Phase Plan

## Phase A: Fixture Expansion (Coverage First)

Primary objective: increase confidence without destabilizing existing parity.

Actions:

1. Expand upstream fixture import from Mermaid `@11.12.2` tests/docs for the most sensitive diagrams:
   - `architecture`, `class`, `mindmap`, `state`, `flowchart`, `sequence`.
2. Keep additions version-pinned and traceable to upstream source path and commit.
3. Add fixtures in small batches and require both global checks green after each batch.

Exit criteria:

- New fixture batches are merged with 0 mismatch in full `parity` and `parity-root` runs.

## Phase B: Override Consolidation (Algorithm First)

Primary objective: convert fixture-scoped overrides to reusable rendering/layout logic where practical.

Priority order:

1. `class` root viewport (smaller fixture count, medium override density)
2. `mindmap` root viewport (small surface area, high leverage)
3. `architecture` root viewport (largest and most layout-sensitive)
4. `state` text/bbox overrides (browser-like HTML/SVG measurement edge cases)

Policy:

- Remove overrides only when replacement logic is deterministic and keeps all existing fixtures green.
- If a removal causes regressions, prefer rollback + follow-up ADR rather than partial drift.

Exit criteria:

- Override count reduced in at least one diagram without introducing parity regressions.

## Phase C: CI Guardrails and Drift Detection

Primary objective: ensure parity does not silently regress.

Actions:

1. Keep mandatory checks for:
   - `compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`
   - `compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`
2. Add a lightweight override inventory report in CI logs (entry count per override file).
3. Document update protocol when pinned Mermaid version changes.

Exit criteria:

- CI rejects parity regressions and makes override growth visible.

## Acceptance Gates

For each PR in this phase:

1. `cargo nextest run`
2. `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`
3. `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`

## Risk Notes

- Root viewport parity is sensitive to browser-like bbox behavior (`svg.getBBox()`, `foreignObject`,
  transformed nested SVG).
- Fixture-scoped overrides are a valid stabilization layer for pinned-version parity, but they increase
  maintenance cost as fixtures grow.
- Prefer deterministic approximation improvements before introducing new broad overrides.

## Backout Strategy

If a hardening change destabilizes parity:

1. Revert the specific algorithmic change.
2. Restore previous override entry if needed.
3. Capture the failed attempt in an ADR or alignment note before the next retry.
