# Host Styling SVG Postprocessors

Status: Complete
Last updated: 2026-05-28

## Why This Lane Exists

The resvg-safe pipeline workstream made generic cleanup explicit. The next downstream pressure point
is host styling: applications need scoped CSS injection, opt-in CSS override behavior, fallback text
that remains readable after `<foreignObject>` conversion, and diagram metadata in custom passes.

Zed PR 57644 is the motivating signal. Zed should not need a large wrapper just to apply app theme
rules, and `merman` should not absorb Zed-specific accent semantics.

## Relevant Authority

- `docs/adr/0063-extensible-svg-output-pipeline.md`
- `docs/adr/0064-host-styling-svg-postprocessors.md`
- `docs/workstreams/resvg-safe-svg-output`
- `docs/rendering/SVG_OUTPUT_PIPELINE.md`
- External signal: `https://github.com/zed-industries/zed/pull/57644`

## Problem

`SvgPipeline` currently exposes a useful custom pass trait, but the implementation is still a single
file mixing public API, preset composition, CSS cleanup, attribute cleanup, and tests. The context
given to host passes only includes preset and pass ordering, so callers must inspect raw SVG to learn
diagram type, title, or root id.

There are no product-neutral styling blocks. A host that only wants scoped CSS injection or an
override policy must write another ad hoc string pipeline.

## Target State

- `pipeline.rs` is split into a `pipeline/` module tree with explicit ownership:
  - `mod.rs`: public `SvgPipeline`, `SvgPostprocessor`, and re-exports.
  - `context.rs`: metadata and `SvgPostprocessContext`.
  - `preset.rs`: built-in preset composition.
  - `builtin/`: foreign object, CSS sanitize, attribute sanitize, scoped CSS, and CSS override.
- `SvgPostprocessContext` exposes diagram type, title, and root SVG id.
- `render_svg_with_pipeline_sync` passes parsed metadata into the pipeline.
- Built-in postprocessors support:
  - scoped CSS injection by root SVG id;
  - opt-in stripping of existing `!important` declarations when host CSS needs to override;
  - fallback text style propagation for generated `<text>` overlays.
- `render_svg_sync` and `SvgPipeline::parity()` remain output-parity contracts.

## In Scope

- `crates/merman-render/src/svg/pipeline/`
- `crates/merman-render/src/svg.rs` public re-exports
- `crates/merman/src/lib.rs` metadata plumbing
- `crates/merman/examples/svg_pipeline.rs`
- `README.md`, `CHANGELOG.md`, `docs/rendering/SVG_OUTPUT_PIPELINE.md`
- Focused render/raster/CLI gates

## Out Of Scope

- Zed-specific accent colors, theme names, or GPUI types.
- Automatic product color assignment for diagram nodes.
- Changing default SVG rendering behavior.
- A fully validating CSS parser.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| Host styling belongs after generic preset processing. | High | Existing `SvgPostprocessor` ordering and raster pipeline. | Add before/after insertion points later. |
| Root SVG id is enough for first-pass CSS scoping. | Medium | Mermaid SVGs already use stable root ids. | Add generated ids or structured root mutation later. |
| String/Cow passes are acceptable for host styling. | Medium | Current trait is already public and simple. | Internal built-ins can move to structured parsing without public API churn. |
| `!important` stripping must be explicit. | High | It changes CSS cascade semantics. | Keep it opt-in and documented. |

## Closeout Condition

This lane can close when module boundaries are split, metadata reaches postprocessors, product-neutral
styling built-ins are documented and tested, examples compile, and focused gates pass with evidence.
