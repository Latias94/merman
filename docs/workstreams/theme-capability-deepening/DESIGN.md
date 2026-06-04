# Theme Capability Deepening - Design

Status: Closed
Last updated: 2026-06-04
Baseline upstream: Mermaid `@11.15.0`

## Problem

`theme-parity` closed the Mermaid 11.15 theme surface gap, but it intentionally stopped short of a
deeper render-side theme architecture. Today `merman-core` owns authoritative `themeVariables`
expansion while `merman-render` still consumes those values through many diagram-local raw JSON
lookups and fallback chains. That keeps parity moving, but it leaves theme capability shallow:

1. theme changes still fan out across many renderer files;
2. diagram CSS providers duplicate the same fallback logic in slightly different shapes;
3. richer capabilities like role-based palettes and accent-derived chart series do not have a
   first-class home between upstream-compatible config expansion and final SVG/CSS emission.

The repository architecture audit already named this seam directly as `ARCH-013`:
"Effective config and theme access is scattered." This lane turns that finding into execution.

## Intent

Deepen render-side theme handling into one explicit module that:

1. preserves `merman-core` as the source of truth for Mermaid-compatible theme expansion;
2. centralizes renderer-facing semantic theme roles and diagram-specific prepared views;
3. deletes repeated raw `themeVariables.*` fallback code from high-duplication CSS consumers;
4. creates a durable place for stronger theme capabilities that still respect Mermaid parity and
   ADR-0064's host-styling boundary.

## Target State

- `merman-core` still expands Mermaid themes and override derivations exactly as before.
- `merman-render` owns a render-side presentation theme module rather than relying on scattered
  direct JSON path reads.
- Shared render theme roles cover the repeated surfaces that currently drift across diagrams:
  typography, primary text, line/border colors, node surfaces, cluster surfaces, note surfaces,
  error colors, label backgrounds, stroke width, and look/theme metadata.
- Diagram families with real extra semantics can request prepared role bundles from the shared theme
  module instead of rebuilding fallback logic locally.
- XYChart-like families have a defined render-side home for accent-derived series palette behavior,
  while explicit Mermaid diagram options still win.
- Host product styling remains outside default parity output and stays in the SVG postprocessor
  pipeline.

## Scope

- `crates/merman-render/src/svg/parity/**`
- `crates/merman-render` renderer tests for touched diagram families
- new ADR for the render-side presentation theme seam
- this workstream's evidence, tasks, and handoff state

## Non-Goals

- Do not rewrite `crates/merman-core/src/theme.rs` into a product-specific theme engine.
- Do not move host palette rewriting, app dark/light policy, or consumer CSS injection into the
  default parity renderer.
- Do not claim full `beautiful-mermaid` feature parity in one lane.
- Do not broaden the public API unless a narrower internal seam cannot support the required
  capability.
- Do not rewrite layout or measurement code unless a theme capability genuinely depends on it.

## Architecture Direction

### Preserve The Current Core Boundary

ADR-0005 and the completed `theme-parity` lane already established a good boundary:
`merman-core` computes effective Mermaid-compatible config and `themeVariables`. This remains the
authoritative compatibility layer.

### Add A Render-Side Presentation Theme View

Introduce a deeper module inside `merman-render` that converts `effective_config` into prepared
theme roles for renderers. The first slice should cover the highest-duplication CSS consumers:

- Flowchart
- Class
- State
- Sequence
- Block

The module should expose:

- base roles shared across many diagrams;
- diagram-specific prepared views only where they delete real duplication;
- a narrow raw-token escape hatch for exact Mermaid-owned CSS values that should not be re-derived.

### Capability Growth Happens Here, Not In Host CSS

The render-side theme module is also the correct place for capability additions that belong to
diagram rendering rather than host styling. The first planned example is accent-derived series
palette support for charts, informed by `repo-ref/beautiful-mermaid`, while keeping explicit
Mermaid `xyChart.*` or `themeVariables.*` values authoritative.

### Respect ADR-0064

Host styling stays a postprocessor concern. This lane may improve render-owned roles and output
quality, but it must not turn parity output into a product palette system.

## Deletion Plan

- Delete repeated `SvgTheme::new(...).color(...)` / `css_value(...)` fallback chains from migrated
- CSS providers and renderer helpers.
- Delete duplicated diagram-local fallback comments once the shared role module makes the behavior
  obvious and tested.
- Delete opportunistic raw `themeVariables` lookups in migrated surfaces when a prepared role view
  replaces them without widening behavior.

## Risks

- Theme refactors can change many SVG outputs at once even when they look behavior-preserving.
- Over-centralizing diagram-specific semantics would create a worse abstraction than the current
  duplication.
- Accent/palette capability work can accidentally cross the parity/host boundary if not kept
  renderer-owned and explicit.

## First Slices

1. Create the lane and ADR, freezing the render-side seam and validation plan.
2. Land the first `PresentationTheme` slice and migrate Flowchart/Class/State/Sequence/Block CSS
   consumers to it.
3. Add chart-oriented palette helpers and migrate XYChart/other inline-theme consumers where the
   new seam removes real duplication.
4. Re-run focused renderer/theme gates and then decide whether broader public/theme docs or binding
   surfaces need a follow-up lane.

## Closeout

This lane closed on 2026-06-04 after completing the planned first slices:

- `PresentationTheme` now owns the first high-duplication SVG/CSS fallback bundles for Flowchart,
  Class, State, Sequence, and Block.
- `chart_palette` now owns XyChart plot palette parsing, explicit-palette precedence, and
  missing-palette derivation.
- HPD-080 public renderability smoke was revalidated through the real integration-test command.

Remaining raw theme access in other diagram families is intentionally not closed here. Mindmap,
GitGraph, Radar, Pie, and other palette-heavy families should be evaluated through narrower
follow-ons only when their Mermaid-specific palette contracts are reviewed.
