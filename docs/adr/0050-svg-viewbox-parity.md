# ADR 0050: SVG ViewBox Parity in Headless Rendering

Date: 2026-01-21

Updated: 2026-08-28

## Context

Mermaid renders SVG using a browser DOM and then derives the final SVG viewport via:

- `setupGraphViewbox(...)`
- `svgElem.node().getBBox()` to compute the rendered bounds
- `svgElem.attr('viewBox', ...)` based on that bounding box
- `configureSvgSize(...)` which sets `width="100%"` and `style="max-width: ...px;"` when `useMaxWidth=true`

In `merman`, we aim for source-backed parity with Mermaid `@11.16.1` while staying headless (no
browser DOM).

Historically, our DOM parity tooling (`xtask` SVG DOM signatures) ignored the root `<svg>` `viewBox`
and `style` attributes in parity modes to reduce noise while iterating on layout and shape output.

However, `viewBox` and root sizing attributes are part of the SVG DOM contract and can regress
without being noticed if they are always excluded from parity checks.

## Decision

1. Keep `parity-root` as the root-aware DOM mode. It compares the normalized descendant tree and
   invokes the blocking root viewport contract for every rendered fixture. The contract rejects
   invalid or non-finite origins/dimensions, non-positive viewports, changed width/height strategy,
   changed non-numeric root style, and changed `max-width`/`viewBox` relationship. Exact origin
   policy remains blocking in the deterministic fixture set, while browser bbox origins remain
   diagnostic.

2. Retain a small exact deterministic fixture set in
   `fixtures/_verification/deterministic-root-contracts.json`. Each row is bound to the pinned
   input and upstream SVG hashes and catches a deterministic root regression without becoming a
   production override or a family tolerance. Admission is restricted to roots whose geometry is
   independent of browser text measurement. Text-driven roots remain under the general viewport
   policy and browser diagnostics instead of pinning either browser floats or fallback heuristics.

3. Treat browser-owned exact bbox values as diagnostics, not routine acceptance policy. The
   schedule/release browser artifact may report exact root and painted-content rectangles together
   with browser identity. A separate browser-mounted cropping oracle checks that painted SVG and
   `foreignObject` descendants remain inside the final viewport using one coordinate-quantization
   epsilon. It does not relax descendant, semantic, or root-contract checks.

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

- `parity-root` provides a blocking guardrail for finite positive dimensions, viewport policy, and
  measurement-independent deterministic examples without requiring browser bbox numerics to be
  stable across fonts and browser versions. Descendant parity remains blocking except where
  ADR-0086 classifies browser-text-measurement layout as diagnostic evidence.
- Some renderers must implement explicit bounding-box logic (including text ascent) to satisfy
  `viewBox` comparisons against upstream baselines.
- Browser-only root movement remains visible in reports and does not require fixture-specific
  production overrides. Exact browser movement is attributable through a schedule/release report,
  while cropping remains independently blocking.

For Mermaid `@11.16.1`, Flowchart root SVG viewport calculation follows the same source-backed
approach by including the diagram title in the headless bounding box before emitting the root
`viewBox`. Browser measurement differences remain an artifact contract rather than a reason to
weaken the shared comparator.
