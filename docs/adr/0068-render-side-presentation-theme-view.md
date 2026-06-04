# ADR 0068: Render-Side Presentation Theme View

- Status: accepted
- Date: 2026-06-03

## Context

`theme-parity` established the current good boundary for Mermaid compatibility:

- `merman-core` computes effective Mermaid config and `themeVariables`;
- `merman-render` consumes that output while preserving parity-oriented SVG emission;
- host styling stays outside the default renderer path and belongs in SVG postprocessors
  (ADR-0063, ADR-0064).

However, renderer theme access is still shallow. Many diagram families read raw JSON paths and
rebuild the same fallback chains independently. The 2026-06-01 architecture audit captured this as
`ARCH-013`: "Effective config and theme access is scattered."

At the same time, `repo-ref/beautiful-mermaid` shows a useful contrast. Its main value is not the
exact color presets; it is the placement of a render-side semantic theme layer between raw config
and final SVG emission. `merman` cannot replace its Mermaid-compatible core config pipeline with
that simplified model, but it can adopt the same architectural depth on the render side.

## Decision

Add a render-side presentation theme view inside `merman-render`.

1. Keep `merman-core` authoritative for Mermaid-compatible theme expansion.
   - `theme`, `themeVariables`, and override derivation remain owned by `merman-core`.
   - This ADR does not move host- or product-specific theme policy into `merman-core`.

2. Introduce a deeper renderer-facing theme module.
   - The module converts `effective_config` into prepared render roles and diagram-oriented views.
   - Shared renderer surfaces such as typography, text roles, borders, surfaces, line colors,
     error colors, note colors, and label backgrounds should come from this module rather than
     repeated raw JSON fallback chains.

3. Allow diagram-specific prepared views only when they delete real duplication.
   - Do not centralize every diagram-local constant or layout rule.
   - When a family truly has extra semantics, it may request a prepared role bundle from the shared
     theme module.

4. Preserve a narrow raw-token escape hatch.
   - Some Mermaid-owned CSS values need exact string interpolation or diagram-local handling.
   - The shared module may expose raw resolved tokens where necessary, but direct raw access should
     become the exception instead of the default.

5. Place renderer-owned capability growth here.
   - Accent-derived series palettes, role-based chart colors, and similar render-time capability
     work belong in this module.
   - Such capability must not mutate core `themeVariables`; explicit Mermaid diagram options and
     explicit theme variables remain authoritative when present.

6. Keep host styling outside the default parity renderer.
   - Product palette injection, app dark/light policy, and host CSS rewriting remain postprocessor
     concerns under ADR-0064.

## Consequences

- Theme changes in shared render surfaces become more local and testable.
- First-order renderer CSS migrations can delete repeated fallback code without weakening parity.
- Capability growth has a named owner between Mermaid-compatible config expansion and final SVG/CSS
  emission.
- The renderer boundary becomes deeper, but only if we avoid turning it into a giant catch-all
  abstraction for diagram-specific behavior.

## Alternatives Considered

1. Keep extending the existing thin `SvgTheme` helper.
   - Pros: smallest patch.
   - Cons: preserves scattered fallback logic and keeps capability growth shallow.

2. Move capability-oriented theme roles into `merman-core`.
   - Pros: one central theme module.
   - Cons: mixes Mermaid compatibility, render semantics, and host concerns into the wrong layer.

3. Solve richer theming with `themeCSS` or host postprocessors only.
   - Pros: avoids renderer refactors.
   - Cons: CSS postprocessing happens too late for many renderer-owned semantics and does not reduce
     raw renderer theme duplication.

4. Copy `beautiful-mermaid`'s entire theme model.
   - Pros: proven role-based design.
   - Cons: it would discard `merman`'s stronger Mermaid config compatibility boundary and would
     blur the distinction between renderer-owned semantics and host styling policy.

## Follow-up

Implement this ADR in `docs/workstreams/theme-capability-deepening`, starting with a first slice
that migrates the highest-duplication SVG/CSS consumers before chart palette capability work.
