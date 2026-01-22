# ADR 0051: Baseline-Derived Font Metrics for Headless Text Measurement

Date: 2026-01-21

## Context

Mermaid `@11.12.2` derives most label dimensions (and therefore layout and the final SVG viewport)
from browser DOM measurements:

- HTML labels are sized based on DOM/layout measurements and then written into the SVG as
  `<foreignObject width="..." height="..."> ... </foreignObject>`.
- Some diagram-level text (e.g. Flowchart titles) is rendered as `<text>` and contributes to
  `svg.getBBox()` when Mermaid computes the final `viewBox` via `setupViewPortForSVG(...)`.

In `merman`, rendering is intentionally headless and Rust-native. We avoid introducing a browser
runtime dependency and we avoid depending on system font discovery as a hard requirement.

However, SVG DOM parity (and ultimately SVG XML parity) for Flowchart and other diagrams is blocked
by text-measurement drift:

- different font availability and fallback across machines,
- sub-pixel rounding differences,
- and heuristic character-width approximations.

## Decision

For Mermaid `@11.12.2`, we will introduce a deterministic, version-scoped text measurement mode
based on **baseline-derived font metrics**:

1. Add an `xtask` generator that reads the pinned upstream SVG baselines and extracts explicit text
   box dimensions that Mermaid already writes into the SVG (primarily `<foreignObject width/height>`
   for HTML labels).
2. Fit a per-character width table (in `em`) for the dominant font-family stacks seen in the
   baselines.
3. Check in the generated Rust table under `crates/merman-render/src/generated/`.
4. Add an optional `TextMeasurer` implementation that uses these tables to compute widths/heights
   deterministically in headless rendering.

This is a pragmatic parity tactic: we reproduce the *effective* metrics used in the pinned baselines
without bundling proprietary font files and without requiring a browser at runtime.

## Alternatives Considered

1. **Full Rust text shaping + font metrics (font parsing + shaping + fallback)**  
   Pros: principled, not tied to pinned baselines.  
   Cons: high complexity; still risks mismatches due to fallback behavior and platform differences;
   requires shipping fonts or relying on system fonts.

2. **Headless browser measurement during rendering**  
   Pros: closest to Mermaid behavior.  
   Cons: breaks headless library constraints and introduces heavy runtime dependencies.

3. **Keep heuristic widths and relax parity requirements**  
   Pros: simplest.  
   Cons: conflicts with “1:1 parity” goals and makes layout/viewport regressions harder to detect.

## Consequences

- Enables incremental convergence on Flowchart `parity-root` by reducing metric-driven layout drift.
- Generated metrics are **version-scoped** and must be re-derived when Mermaid baselines change.
- Metrics are only as good as the baseline coverage; rare characters/fonts may still fall back to
  heuristics.

## Follow-ups

- Integrate the new measurer into `xtask compare-*` commands to quantify parity improvements.
- Decide when to make the baseline-derived measurer the default (after regenerating internal golden
  snapshots if needed).
- Extend extraction beyond Flowchart if other diagrams are blocked by the same metric drift.

