# ADR-0063: Extensible SVG Output Pipeline

## Status

Accepted

## Context

`merman` optimizes for Mermaid parity. That is the right default for golden SVG comparison, but
downstream applications often need a different output contract:

- UI previews and raster export need readable output when the renderer does not support
  `<foreignObject>`.
- `usvg` / `resvg` consumers need unsupported CSS and invalid visual attributes removed.
- Product integrations may need app-specific theming, accent colors, filtering, or CSS injection.

Zed PR 57644 (`markdown: Merman`) integrated `merman` through a new internal
`crates/mermaid_render` wrapper. The wrapper depends on crates.io `merman 0.4`, then applies a
large SVG post-processing pipeline for CSS cleanup, fallback text, accent colors, and
`usvg` / `resvg` compatibility. That is strong evidence that `merman` should own the generic
extension boundary, while allowing host applications to keep product-specific passes outside the
core crate.

The Zed wrapper is GPL-licensed inside Zed. We should treat it as requirements evidence, not code
to copy into this MIT/Apache workspace.

## Decision

Keep parity SVG output as the default and introduce a separate, explicit SVG output pipeline for
consumer-oriented output.

The intended shape is:

- `SvgPipeline::parity()` preserves the current Mermaid-like output.
- `SvgPipeline::readable()` adds best-effort text fallback for labels that use `<foreignObject>`.
- `SvgPipeline::resvg_safe()` adds readable output plus compatibility cleanup for
  `usvg` / `resvg` and raster/PDF export.
- A public `SvgPostprocessor` trait lets host applications append product-specific output passes.

The first public postprocessor API should be string-oriented:

```rust
pub trait SvgPostprocessor: Send + Sync {
    fn name(&self) -> &'static str;
    fn process<'a>(
        &self,
        svg: std::borrow::Cow<'a, str>,
        ctx: &SvgPostprocessContext<'_>,
    ) -> Result<std::borrow::Cow<'a, str>>;
}
```

Do not expose a low-level XML event iterator in the first version. The internal implementation may
use a streaming XML parser for built-in passes, but exposing that shape would prematurely lock
lifetimes, dependencies, and ordering semantics into the public API.

## Consequences

- Parity fixtures remain meaningful because the default SVG path is not silently cleaned for
  product display.
- Raster/readable output can improve without each host copying ad hoc cleanup logic.
- Hosts like Zed can keep application-specific accent/theme passes without forking `merman`.
- Public API review must treat pass ordering, error handling, and ownership as semver-significant.
- A future advanced event-stream API remains possible if profiling proves string-oriented custom
  passes are a real bottleneck.
