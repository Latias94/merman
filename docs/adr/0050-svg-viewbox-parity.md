# ADR 0050: SVG ViewBox Parity in Headless Rendering

Date: 2026-01-21

## Context

Mermaid renders SVG using a browser DOM and then derives the final SVG viewport via:

- `setupGraphViewbox(...)`
- `svgElem.node().getBBox()` to compute the rendered bounds
- `svgElem.attr('viewBox', ...)` based on that bounding box
- `configureSvgSize(...)` which sets `width="100%"` and `style="max-width: ...px;"` when `useMaxWidth=true`

In `merman`, we aim for 1:1 parity with Mermaid `@11.12.2` while staying headless (no browser DOM).

Historically, our DOM parity tooling (`xtask` SVG DOM signatures) ignored the root `<svg>` `viewBox`
and `style` attributes in parity modes to reduce noise while iterating on layout and shape output.

However, `viewBox` and root sizing attributes are part of the SVG DOM contract and can regress
without being noticed if they are always excluded from parity checks.

## Decision

1. Introduce a new DOM signature mode: `parity-root`.
   - Same as `parity` (masks geometry noise and ignores `<style>` content).
   - Additionally compares the root `<svg>` `viewBox` and `style` attributes.

2. For diagrams that use Mermaid's `setupGraphViewbox` behavior (e.g. Sankey), implement headless
   bounding-box calculation that includes text ascent so that `viewBox` can match the upstream
   baselines within the configured DOM numeric rounding (`--dom-decimals`).

## Alternatives Considered

1. **Keep ignoring root `viewBox` in parity checks**  
   Pros: fewer diffs while iterating.  
   Cons: silently regresses size/viewBox behavior, slowing down true 1:1 alignment work.

2. **Use `strict` mode everywhere**  
   Pros: maximum DOM scrutiny.  
   Cons: too brittle at this stage because Mermaid emits large, environment-sensitive `<style>` blocks
   and many diagrams still rely on incremental parity work.

3. **Full Rust text shaping + font metrics**  
   Pros: closest to browser measurement.  
   Cons: high complexity; still risks mismatches due to fallback fonts, rendering engines, and
   platform differences.

## Consequences

- `parity-root` provides a stronger guardrail for SVG size and `viewBox` parity without requiring
  full CSS parity.
- Some renderers must implement explicit bounding-box logic (including text ascent) to satisfy
  `viewBox` comparisons against upstream baselines.
- This is an incremental step toward full SVG XML parity while keeping the headless design goals.

For Mermaid `@11.12.2`, Flowchart root SVG viewport calculation now also follows this approach by
including the diagram title in the headless bounding box before emitting the root `viewBox`.
