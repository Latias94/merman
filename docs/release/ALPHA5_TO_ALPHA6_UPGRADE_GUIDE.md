# Upgrade from 0.8.0-alpha.5 to 0.8.0-alpha.6

> [!IMPORTANT]
> Alpha.6 is currently a prepared prerelease candidate. This document does not imply that any
> registry package, tag, or platform artifact has been published.

Alpha.6 deliberately breaks the prerelease ASCII API so terminal rendering can expose one coherent
semantic, resource, and capability contract. Upgrade generated wrappers together with their native
or WASM artifact; versions from alpha.5 and alpha.6 are not wire-compatible.

## Rust ASCII API

- `AsciiRenderOptions::max_grid_cells` and `with_max_grid_cells(...)` are removed. Select an
  `AsciiResourcePolicy` with `with_resource_policy(...)` or `with_resource_profile(...)`. To replace
  the old builder directly, use
  `with_resource_limit(AsciiResourceLimitId::MaxGridCells, value)?`.
- The policy now owns independent limits for grid cells, layout work, document cells, encoded
  output bytes, grapheme bytes, and nesting depth. A grid limit is not a proxy for later phases.
- `AsciiError::RenderLimitExceeded { actual, limit }` is replaced by
  `AsciiError::ResourceLimitExceeded(AsciiResourceLimitExceeded)`. Match its typed `limit`,
  `actual`, `max`, and `profile`; use `phase()` for the typed failure phase. Post-admission
  allocation failures use `AsciiError::AllocationFailed`.
- `AsciiResourceLimitDescriptor::phase` is now `AsciiResourceLimitPhase`, and every descriptor has
  a typed `id`. Use `phase.as_str()` and `stable_id` only at display or serialization boundaries.
- `HeadlessAsciiError` no longer exposes authored text through an error source chain. Match the
  public variant and use `terminal_diagnostic_details()` for safe structured diagnostics.
- `AsciiRenderOptions` now carries `terminal_width_profile`. Use its constructors and builders so
  Unicode and CJK width behavior remains explicit.

## Typed render models

- `merman_core::diagrams::flowchart::FlowEdge` now carries independent `start_marker`,
  `end_marker`, `stroke_kind`, and `visibility` fields in addition to the authored arrow. Legacy
  serialized values remain readable, but Rust struct literals must initialize the complete shape.
- `ErDiagramRenderModel::{classes, entities}` now use declaration-ordered `indexmap::IndexMap`
  instead of `std::collections::BTreeMap`. Update explicit annotations, constructors, and helper
  signatures.

## Capability discovery

ASCII support is no longer one undifferentiated label. Read both dimensions:

- `semantic_coverage` states whether the admitted projection is full or partial.
- `primary_projection` distinguishes `diagrammatic` output from `structured-text` output.

The compatibility `support_level` is derived from those values. The old `summary_fallback` field is
renamed to `structured_text_fallback`. Do not count a structured-text family as a diagram merely
because it produces useful terminal output. The six primary diagrammatic families remain Partial:
Flowchart, Sequence, State, Class, ER, and XYChart.

## UniFFI and browser WASM

- UniFFI binding API advances from `3` to `4`.
- Browser WASM transport API advances from `3` to `4`.
- Generated ASCII capability records expose `semantic_coverage`, `primary_projection`, and
  `structured_text_fallback`.
- Structured resource diagnostics expose stable typed limit, phase, cause, observed, and maximum
  fields. Do not classify failures by parsing display text.

Upgrade the Python or Apple wrapper together with its alpha.6 UniFFI library. Likewise, upgrade an
`@mermanjs/web*` package together with its owned alpha.6 WASM artifact. Runtime version checks are
expected to reject mixed artifacts.

## Output compatibility

ASCII snapshots can change even when the Mermaid source does not. Alpha.6 fixes graph direction,
compound ownership, parallel and self-loop routing, Sequence signal/control semantics, Class/ER
endpoint roles, State notes and compartments, and XYChart coordinates. Treat those changes as
semantic corrections and regenerate byte snapshots after reviewing the resulting topology and
fact disclosures.

Families whose terminal value is primarily a report remain StructuredText. Unsupported families
continue to fail explicitly; they do not silently return a lossy summary.

## Timeline and Journey direct models

`TimelineRenderTask` and `JourneyRenderTask` now carry an optional `section_index` occurrence
owner. Parser-produced tasks always point at the section occurrence that authored them, so repeated
section labels remain distinct. Direct-model callers should set `section_index` when the label is
ambiguous; a unique legacy label may remain `None` and is inferred. Unknown, orphan, and empty
sections are retained in the structured-text projection with explicit `[undeclared]` or
`[unsectioned]` markers instead of being silently dropped.
