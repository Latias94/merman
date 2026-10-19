---
title: Theme designer product direction
date: 2026-10-19
status: proposed
---

# Theme designer product direction

## Problem and intended outcome

The current workbench exposes a useful subset of `ThemeRecipeV1`, but its information architecture
still follows the wire format. Users must understand semantic target IDs, family IDs, nullable
patches, ordered rules, and two recipe representations before they can explore a visual idea.
Adding more fields to the same long form would increase capability without improving discovery.

The intended outcome is a theme creation journey: choose a starting style, adjust its appearance,
refine semantic objects, inspect representative diagrams, and save a reusable recipe. A user should
complete the ordinary journey without typing identifiers or JSON. Advanced controls should expose
the renderer's real capabilities rather than confining most of them to the source editor.

This document proposes the next product iteration. Only the immediate repairs below are implemented
in this change; the remaining sections describe future work, not shipped behavior.

## Immediate repairs

- Replace the fixed desktop `lg:w-96` sidebar with the existing resizable panel primitive. Support
  pointer and keyboard resizing, remember desktop and stacked layouts separately, and retain
  enough workspace for the diagram. Stacked layouts resize vertically.
- Replace family and variant text inputs with runtime-backed selections. Preserve existing values
  that a newer runtime may understand. Append overrides by choosing a named family directly.
- Keep layout preferences outside the recipe, workspace link, and render-operation identity.

## Reference products and the useful interaction

These are interaction references; their style engines and document formats do not define Merman's
semantics. Sources were read on 2026-10-19.

| Product | Observed behavior | Application to Merman |
| --- | --- | --- |
| [Radix Themes Playground](https://www.radix-ui.com/themes/playground) | Accent/gray choices, light/dark appearance, radius, scaling, and immediate component specimens | Give users a small set of discoverable global choices with visible results |
| [draw.io shape styles](https://www.drawio.com/docs/manual/styles/shape-styles/) | Selection opens an appropriate property panel; palettes, gradient controls, border styles, and text tools precede raw style editing | Present semantic objects first, then contextual visual properties; keep advanced source editing available |
| [Penpot Design Tokens](https://help.penpot.app/user-guide/design-systems/design-tokens/) | Reusable typed values, explicit token binding/detaching, sets/themes, import/export | Make shared values and custom overrides explicit; do not infer bindings from equal colors |
| [VS Code Settings](https://code.visualstudio.com/docs/configure/settings) | Searchable grouped settings and JSON editing | Search/group advanced controls while retaining one underlying document |

Borrow the interaction patterns without implementing a general vector editor or another token
standard. Merman remains a semantic diagram theme authoring tool.

## Recommended workspace

Use a dedicated design mode within Playground, sharing the existing edit session and rendering
pipeline. Source editing can be collapsed while designing. The central diagram should receive most
of the space; the property panel is resizable. On phones, keep a compact preview above a vertically
resizable property panel. Do not make source, preview, and a fixed sidebar compete for equal space.

The designer has two primary entry points: **Start** and **Design**. Move execution details and JSON
under an advanced/tools entry rather than presenting them as equally prominent creative tasks.

Suggested arrangement:

```text
Theme name / starting preset     Undo  Redo     Save / Export
-----------------------------------------------------------
                                | Design
     Current diagram            | Colors
     or chosen sample           | Typography
                                | Shapes and lines
                                | Background layers
                                | Effects
                                | Family overrides
-----------------------------------------------------------
Current diagram / sample        | Advanced: rules, JSON, outcome
```

These sections describe tasks, not new serialization owners. The same authored document supplies
visual editing, raw JSON, import/export, and the actual renderer.

## Creation journey

1. **Choose a starting point.** Show the loaded presets and a neutral starter. Choosing Edit creates
   an independent editable copy in one action. Distinguish theme saving from applying it to the
   current diagram. Use “New theme” in the ordinary UI; keep “recipe” in technical documentation.
2. **Adjust the shared appearance.** Offer color swatches/recent colors, font-family previews,
   weight choices, numeric fields paired with sliders, and explicit field reset. Retain precise
   values and custom font stacks under an advanced disclosure.
3. **Refine semantic objects.** Let users choose Nodes, Connections, Labels, Groups, Actors, Notes,
   Series, or other family-relevant targets. Use friendly labels and search; retain raw IDs in
   technical details. Scope is a visible choice: all families or a named family.
4. **Compose background and effects.** Edit gradients, patterns, layers, shadows, and glow with
   specialized controls. Show a preview before applying a component style and allow its parameters
   to be changed afterward. This is essential creative scope, not optional JSON-only polish.
5. **Try another diagram.** Offer the current diagram plus a small curated sample picker. Render
   only the selected sample, on demand, with the same theme and renderer. Keep sample source separate
   from the user's workspace. Results describe that sample; they are not a certification matrix.
6. **Save and reuse.** Download/copy the standalone recipe and share an applied workspace using the
   existing contracts. A local saved-theme shelf and duplication can follow; neither requires a
   cloud account or a different public theme format.

Exploration needs bounded undo/redo across Visual and JSON, field reset, rule duplication, and
component removal. Commit slider drags as one history action. Preserve the current Apply/Cancel
contract and do not accidentally apply unrelated source/configuration edits.

## Increase visual freedom using existing capabilities

| Area | Current renderer/recipe capability | Proposed visual control |
| --- | --- | --- |
| Paint | Solid/transparent, linear/radial gradients, repeated/tiled gradients, dots/grid/stripe patterns | Paint-kind picker, gradient stops with positions, angle/center/radius, repetition and pattern spacing |
| Canvas | Ordered layers, opacity, blend mode, offsets, bleed | Layer list with add, duplicate, reorder, remove, and individual parameters |
| Shape/line | Fill and stroke opacity, width, dashes, cap/join, radius, padding | Swatches, sliders with precise input, line-pattern choices, corner/spacing controls |
| Typography | Font stack/size/weight/style, line height, spacing, transformation, decoration, alignment/wrapping | Font/weight menus, typography controls grouped by purpose; show conditional support without inventing it |
| Series | Ordinal palettes and exact/cyclic selectors | Reorderable color chips, add/remove color, explicit series/ordinal scope |
| Effects | Ordered graphs with shadow, blur, color matrix, turbulence, displacement and bindings | Start with editable shadow/glow templates and effect binding; disclose graph composition as advanced |
| Rules | Target, family, variant, ordinal, ordered per-facet overrides | Named scopes, searchable rule list, duplicate/reorder/reset; ordinal controls rather than JSON fragments |

The breadth of this contract exceeds the current UI. The immediate freedom problem is mostly an
authoring interface gap, although execution support still varies by family, target, look, and
output. Do not claim every property works everywhere because it can be serialized.

## Boundaries that shape the design

### Shared color editing must have a real binding

`definition` stores a compact set of editable color/typography tokens. Exported presets are
`complete_spec` snapshots, with concrete rules, palettes, paints, and effect values. There is no
general reversible token/source map in that snapshot. Matching identical color strings does not
establish that they represent the same design role.

For a definition, edit the actual tokens. For a complete snapshot, show direct semantic styles and
explicitly selected groups of authored properties. A later one-control “brand color” that reliably
updates a complex preset requires an explicit role/reference authoring model and a versioned
materialization contract. Keep that as a separate, evidence-backed schema decision. Do not add
undeclared metadata to V1 recipes or infer hidden inheritance in the frontend.

### Advanced paints require the complete representation

Definition color tokens accept strings. Adding a gradient canvas or effects needs a complete
specification. Offer an explicit “Enable advanced styling” conversion through `materializeTheme`,
preserve the prior document in undo history, and keep rendering through Rust. Conversion does not
permit reconstructing the former token model by reverse inference.

### Picking an object does not imply an individual-object selector

V1 selectors identify target/family/variant/ordinal, not arbitrary Mermaid node IDs. A future canvas
picker needs renderer-owned semantic metadata, not CSS selectors guessed from SVG IDs or classes.
After selection, state the exact scope (for example, all nodes in the current family). Do not imply
that one clicked node can receive a theme rule that the contract cannot express. Keep arbitrary
per-object source styling separate from reusable theme creation.

### Appearance is not geometry or layout ownership

Themes do not change Mermaid syntax, graph topology, layout selection, or node shape kinds. Radius
only changes channels actually consumed by the selected shape writer. Likewise, an effect template
must not silently change Flowchart's look to Classic. Explain unmet conditions with an explicit
configuration action when appropriate.

### Honest support with useful feedback

Prefer feedback beside the affected property and actual rendered result. Preserve conditional,
unknown, and unsupported outcomes; a discovery response is not a trace of winning authored fields.
Retain `qualified_cells` in technical metadata and keep it out of creation gates. Background preview
backing remains a display preference; canvas paint belongs to the exported image.

## Suggested delivery order and acceptance

1. **Ergonomics:** resizing, runtime selections, typography presets, grouped controls, and document
   undo/redo. Users can restyle a neutral starter without entering a schema ID or losing a draft.
2. **Visual composition:** paint editor, canvas layers, line styles, series palettes, and editable
   shadow/glow templates. Users can build a patterned/gradient background and apply a scoped effect
   without JSON; untouched imported structure survives save/import.
3. **Reusable creation:** preset starting journey, chosen-sample preview, local theme shelf, and
   explicit reusable role bindings if justified. Samples never overwrite source; saved recipes
   remain accepted by the same Rust compiler and exchange API.

Implement each slice with the existing render pipeline and scoped browser checks. Do not add eager
all-preset/all-family renders, a parallel CSS theme engine, a certification workflow, or a general
design-system framework as prerequisites.

## Local evidence

- `playground/src/App.tsx`: workspace/preview layout and the previous fixed sidebar.
- `playground/src/components/theme-workbench/ThemeWorkbench.tsx`: one draft, Apply/Cancel, import/
  export, runtime catalog, and exact-render metadata.
- `playground/src/components/theme-workbench/ThemeVisualEditor.tsx`: current visual subset and
  fields that previously exposed raw identifiers.
- `platforms/web/src/public-types.ts`: `ThemeRecipeV1`, `ThemeDefinitionV1`, `DiagramThemeSpec`,
  `ThemePaint`, `ThemeCanvasSpec`, `ThemeEffectEntry`, and selectors.
- `docs/rendering/custom-diagram-themes.md`: output conditions, ownership, and qualification.
- `docs/plans/2026-10-16-refactor-demand-driven-theme-rendering-plan.md`: demand-driven runtime
  boundaries retained by this proposal.
