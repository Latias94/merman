# ADR 0064: Host Styling SVG Postprocessors

Date: 2026-05-28

## Status

Accepted

## Context

ADR 0063 introduced `SvgPipeline` so consumer-oriented SVG cleanup does not mutate Mermaid-parity
rendering by default. Zed PR 57644 then exposed the next boundary: product hosts need to inject
their own SVG styling, override upstream CSS safely, and inspect diagram metadata while doing so.

Those needs are broader than `resvg_safe` cleanup. Treating them as more `resvg_safe` behavior would
mix three contracts:

- parity SVG, which should keep matching Mermaid output;
- generic consumer cleanup, which makes SVG readable or raster-safe;
- host product styling, which depends on app theme, accent colors, and UI semantics.

## Decision

Model host styling as postprocessors layered on top of the existing pipeline:

1. `SvgPipelinePreset` stays responsible for generic output targets: `Parity`, `Readable`, and
   `ResvgSafe`.
2. Built-in postprocessors provide host-useful but product-neutral blocks, including scoped CSS
   injection, opt-in CSS override behavior, and fallback text style propagation.
3. Host applications keep product-specific semantics in custom `SvgPostprocessor` implementations.

`SvgPostprocessContext` will expose read-only metadata so host passes do not need to parse or guess:

- selected preset;
- pass index and pass name;
- diagram type;
- diagram title;
- root SVG id.

Default `render_svg_sync` remains parity output. Styling and override behavior only happen when the
caller selects a pipeline or appends postprocessors.

## Consequences

- `merman` can support Zed-like integration needs without copying Zed-specific logic or GPL code.
- Public pass ordering becomes part of the API contract: built-in preset behavior runs first, then
  appended host postprocessors in insertion order.
- Host styling examples can evolve independently from `resvg_safe` raster compatibility.
- The string/Cow postprocessor trait remains the stable first layer; structured XML/CSS parsing can
  be added later behind the built-ins if profiling or correctness evidence requires it.

## Non-Goals

- Do not encode Zed `player_colors`, `zed-accent-N`, GPUI color types, or dark/light theme models in
  `merman`.
- Do not inject host CSS or strip `!important` by default.
- Do not change Mermaid-parity SVG output from `render_svg_sync`.
- Do not expose a low-level XML event-stream API in this lane.
