# Resvg-Safe SVG Output Pipeline

Status: Active
Last updated: 2026-05-28

## Why This Lane Exists

`merman` currently has a strong parity renderer and a small readable fallback helper, but common
UI/raster consumers still need to build their own SVG cleanup layer. Zed PR 57644 exposed this
clearly: Zed depends on crates.io `merman 0.4`, then wraps it in an internal `mermaid_render`
crate with multiple post-processing passes for CSS cleanup, fallback text, accent colors, and
`usvg` / `resvg` compatibility.

That is not a reason to copy Zed's GPL code. It is evidence that `merman` needs a first-class
output pipeline boundary.

## Relevant Authority

- ADRs:
  - `docs/adr/0004-public-api-and-headless-output.md`
  - `docs/adr/0059-raster-output-strategy.md`
  - `docs/adr/0063-extensible-svg-output-pipeline.md`
- Existing docs:
  - `docs/workstreams/ALIGNMENT.md`
  - `docs/workstreams/PARITY_BOUNDARY.md`
  - `docs/rendering/RASTER_OUTPUT.md`
- External signal:
  - `https://github.com/zed-industries/zed/pull/57644`
  - `repo-ref/zed/crates/mermaid_render`

## Problem

The current public surface mixes three concerns:

- Mermaid-parity SVG rendering.
- Best-effort readable fallback for `<foreignObject>` labels.
- Raster/PDF compatibility cleanup.

The cleanup path is too narrow for host applications and too implicit for long-term API stability.
Downstreams that need custom theming or stricter renderer compatibility must either fork, wrap, or
copy a large post-processing pipeline.

## Target State

- Parity SVG output remains the default and does not silently apply consumer cleanup.
- Readable/raster output flows through an explicit `SvgPipeline`.
- Built-in presets cover `Parity`, `Readable`, and `ResvgSafe`.
- Host applications can append custom `SvgPostprocessor` passes without depending on internal
  render modules.
- Raster, PDF, and readable SVG helpers share the same pipeline implementation.
- Zed-inspired regression tests cover the generic failures observed in their integration work.

## In Scope

- `crates/merman-render/src/svg/parity/fallback.rs`
- New SVG post-processing modules under `crates/merman-render/src/svg/`
- Public `merman::render` wrapper APIs and `HeadlessRenderer` builder methods
- Raster/readable helper routing in `crates/merman/src/lib.rs` and
  `crates/merman/src/render/raster.rs`
- Tests for fallback text, invalid SVG attributes, unsupported CSS, and `usvg` / `resvg` smoke
- README and changelog updates

## Out Of Scope

- Copying Zed's GPL implementation.
- Making Zed theme or player-color semantics part of `merman`.
- Replacing the parity SVG renderer.
- Changing Mermaid-parity DOM output by default.
- Exposing a low-level XML event streaming public API in the first iteration.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| Parity SVG and consumer SVG need different defaults. | High | Current golden parity docs and Zed wrapper. | We would overfit parity output to product display. |
| A string/Cow postprocessor trait is enough for first public API. | Medium | Custom passes are expected to be host-specific and fewer than built-ins. | Add an advanced event-stream feature later. |
| Built-in resvg-safe cleanup belongs in `merman`, not every downstream wrapper. | High | Raster helpers already use fallback text; Zed duplicated broader cleanup. | Downstreams continue to fork or wrap heavily. |
| Public pass ordering is semver-relevant. | High | Theme CSS, fallback text, and strip-foreignObject passes depend on order. | API must make ordering explicit before stabilization. |

## Architecture Direction

Create a small pipeline abstraction above the existing parity renderer:

1. Render Mermaid-parity SVG exactly as today.
2. Apply a selected `SvgPipeline` preset.
3. Run host-provided postprocessors in deterministic order.
4. Feed the resulting SVG to raster/PDF exporters when needed.

The initial public extension trait should operate on `&str` and return `Cow<str>`. Built-in
passes can use structured parsing internally, but dependency and lifetime details stay private.

## Closeout Condition

This lane can close when:

- `SvgPipeline` presets exist and are documented.
- Readable and raster/PDF helpers route through the pipeline.
- At least one public custom postprocessor test proves extension ordering.
- Zed-inspired generic regressions are covered without copying GPL implementation.
- Focused render/raster gates pass and evidence is recorded.
