# Theme Parity Refactor - Design

Status: Complete
Last updated: 2026-06-01
Baseline upstream: Mermaid `@11.12.3`

## Problem

Merman currently has partial Mermaid theme support. `theme: default` is not expanded through the
same path as other themes, renderer modules carry repeated fallback defaults, and the Web/WASM
surface maintains its own theme list separately from core behavior. This makes SVG parity fragile:
small host styling or theme-variable gaps can change rendered text, fills, strokes, or compare-mode
output.

## Intent

Make theme handling a single explicit pipeline:

1. Core config expands Mermaid-compatible theme variables.
2. Renderers consume resolved theme variables through a small shared resolver.
3. WASM and TypeScript expose only the themes that core can expand.
4. Playground and Mermaid compare mode use the same supported-theme source.

The refactor should delete redundant renderer and frontend fallback code once the core path makes
those defaults authoritative.

## Target State

- `merman-core` expands `default`, `base`, `dark`, `forest`, and `neutral` theme variables.
- Theme overrides remain user-controlled: explicit `themeVariables` values win after derived values
  are calculated, matching Mermaid's two-pass override behavior where relevant.
- `merman-render` has a shared theme resolver for SVG parity code instead of repeated ad hoc JSON
  lookups and hardcoded color/font fallbacks.
- Common SVG CSS emission is scoped and does not rely on host-page inherited text color.
- Mermaid `themeCSS` is supported as diagram-owned CSS after parity SVG rendering, scoped to the
  root SVG id. Unsupported top-level at-rules are dropped, nested grouping rules are scoped, and
  raster-safety cleanup remains the responsibility of `SvgPipeline::resvg_safe()`.
- The Web/WASM theme list is derived from one source and includes no theme that renderers cannot
  reasonably represent.

## Scope

- `crates/merman-core/src/theme.rs`
- `crates/merman-core` config tests
- `crates/merman-render/src/svg/parity/**`
- `crates/merman-wasm/src/lib.rs`
- `platforms/web/src/**`
- `playground/src/**`
- Theme-focused Rust, WASM, and frontend tests
- Workstream evidence and changelog notes

## Non-Goals

- Do not implement every Mermaid theme in the first pass.
- Do not expose `neo`, `neo-dark`, `redux`, `redux-dark`, `redux-color`, or `redux-dark-color`
  until their visual semantics are covered or clearly labeled experimental.
- Do not rewrite diagram layout algorithms for theme work.
- Do not delete upstream fixtures or parity baselines to reduce failures.
- Do not accept unscoped user CSS into SVG output.

## Architecture Direction

### Core Theme Expansion

`apply_theme_defaults` becomes the single place where supported Mermaid theme presets are expanded
into `themeVariables`. The first priority is `default`, because Mermaid always expands it during
initialization when no explicit supported theme is provided.

### Render Theme Resolver

SVG parity renderers should consume a narrow resolver for common values:

- font family
- font size
- root text color
- node/background/border colors
- label background
- class/block/flowchart text roles

Diagram-specific values stay in diagram modules when they are genuinely layout or renderer
defaults rather than Mermaid theme variables.

### CSS Pipeline

Shared CSS generation should own root SVG font/text defaults and scoped user-facing CSS. Existing
diagram-specific CSS modules can remain, but common CSS should not be copy-pasted into each
renderer.

### Frontend Surface

The supported theme list should be single-source. The TypeScript wrapper and playground may use a
static type for ergonomics, but it must agree with the WASM-exported list and Mermaid compare mode.

## Deletion Plan

- Delete renderer fallbacks that duplicate core theme defaults after the core expansion is covered
  by tests.
- Delete duplicate frontend theme lists and normalization branches once a shared constant or WASM
  source exists.
- Delete comments that describe theme defaults as minimal stopgaps when the implementation becomes
  authoritative.
- Delete empty or redundant `<style>` emission paths where a scoped CSS builder replaces them.

## Risks

- Theme expansion can change many SVG outputs at once. Use targeted tests before broad parity gates.
- Some Mermaid theme values come from `khroma` color operations; exact serialization matters for
  golden SVG comparisons.
- `themeCSS` is a security-sensitive surface. Diagram directives are still sanitized during
  preprocessing; scoped CSS injection prevents host-page selector leakage, while raster-safety
  cleanup remains opt-in through the SVG pipeline.
- Frontend theme changes can affect shared URLs and history entries. Unknown themes should degrade
  to `default`.

## Follow-Ups

- Add broad theme parity fixtures for Flowchart, Class, Block, and ER across
  `default/base/dark/forest/neutral` plus overrides.
- Continue migrating remaining diagram-specific theme reads to shared resolver helpers where that
  removes duplication without weakening parity.
- Treat Mermaid `neo` and `redux` theme families as a separate design lane because their visual
  semantics are broader than color-variable expansion.
