---
title: Pele theming and Merman theme authoring
date: 2026-10-19
status: researched
---

# Pele theming and Merman theme authoring

## Recommendation

Borrow Pele's small, recognizable semantic color vocabulary and clear distinction between
global values and local diagram styles. Keep Merman's existing typed design document,
explicit references, Rust materialization, and native export ownership. The strongest next
improvement is a simpler presentation of existing authoring capabilities, rather than another
theme serialization model.

This recommendation is an inference from the first-party evidence below and Merman's
[ADR 0095](../adr/0095-general-theme-authoring-and-retained-bindings.md) and
[authoring design](2026-10-19-general-theme-authoring-interface.md). It is not a claim that
Pele provides a visual theme designer or portable theme JSON.

## Scope and current status

Research checked the official site and repository on 2026-10-19. Repository source links
below pin commit `44da152bbb79b91f03e68016f42499bb25bf6e82`. This investigation concerns
theme design; it does not propose adding Pele as a dependency.

The evidence is documentation and static source inspection. This research did not execute Pele,
exercise its playground in a browser, or visually compare its output. Absence of a documented
theme JSON/editor feature is not proof that no external integration could provide one.

The repository belongs to `obsidianmd`; the site credits `kepano`. Steph Ango's own biography
identifies him as kepano and Obsidian's CEO. His Obsidian project page says he first
participated as a community contributor and became CEO in 2023. These sources support the
Obsidian connection, but not describing him as Obsidian's original author.
[Repository](https://github.com/obsidianmd/pele),
[biography](https://stephango.com/about), [Obsidian project](https://stephango.com/obsidian)

## What Pele actually does

### A small semantic vocabulary, with local escape hatches

The official theming guide exposes semantic variables for text, secondary text, lines,
surfaces, borders, highlights, critical states, fonts, weights, and radius, plus eight cycling
series colors. Variables can be set on the SVG or its ancestors. Named SVG classes and
`data-id` provide local styling hooks. Literal styles written in diagram source override
theme colors and remain fixed when the theme changes. This is a compact public vocabulary
backed by ordinary CSS inheritance, not a documented JSON authoring graph.
[Theming guide](https://pele.run/theming)

The implementation resolves public variables to short private aliases at the SVG root,
retaining built-in fallback values. `seriesColor()` cycles the eight public series variables.
The critical-state color is deliberately separate from the ordinal series palette. The same
module separates source declarations into shape/line/text properties and admits a finite
property allowlist. That separation is a useful precedent for explaining body versus label
styles without requiring users to know emitted SVG structure.
[Theme implementation](https://github.com/obsidianmd/pele/blob/44da152bbb79b91f03e68016f42499bb25bf6e82/src/svg/theme.ts)

For example, flowchart node bodies consume surface and border roles; clusters consume an
alternate surface and muted label role. Each family renderer owns this mapping. A semantic
token therefore changes several meaningful consumers, while the document's source styles
can still replace individual properties.
[Flowchart renderer](https://github.com/obsidianmd/pele/blob/44da152bbb79b91f03e68016f42499bb25bf6e82/src/diagrams/flowchart/render.ts)

### Light/dark themes are host CSS assignments

The website supplies explicit light and dark Flexoki mappings, including complete series
colors, under `data-pele-theme` selectors. Fonts and radius are mapped from the site's own
variables. This demonstrates an embedding adapter: the host's design system supplies
values for the library's semantic roles. It does not establish automatic palette derivation,
contrast correction, named preset inheritance, or a downloadable theme document.
[Website theme mappings](https://github.com/obsidianmd/pele/blob/44da152bbb79b91f03e68016f42499bb25bf6e82/website/src/styles/pele-theme.scss)

### Paint changes and font changes have different costs

Color variables remain in the SVG, allowing browser CSS to recolor existing markup. Font
changes are different: `mount()` obtains fonts from computed styles, and `refresh()` checks
whether fonts or available width require rendering again. Pending web-font completion
invalidates measured widths. This is useful evidence for separating paint-only changes from
measurement dependencies; it is not evidence that every theme setting can change without
layout work.
[Mount implementation](https://github.com/obsidianmd/pele/blob/44da152bbb79b91f03e68016f42499bb25bf6e82/src/mount.ts),
[public API](https://pele.run/api)

### A background role is not a transparency switch

The root SVG writer establishes color/font aliases and document structure; it does not
unconditionally insert a full-canvas background rectangle. Separately, pie-chart output uses
the background role for slice separators and labels inside slices. Thus replacing the
background role with transparent would also affect foreground details. Merman should retain
distinct concepts for preview backing, exported canvas paint, node surfaces, and text on
colored surfaces.
[SVG root writer](https://github.com/obsidianmd/pele/blob/44da152bbb79b91f03e68016f42499bb25bf6e82/src/svg/root.ts),
[Pie renderer](https://github.com/obsidianmd/pele/blob/44da152bbb79b91f03e68016f42499bb25bf6e82/src/diagrams/pie/render.ts)

### Its playground is a diagram editor, not a theme-authoring reference

The inspected playground has source/output views, source-file opening, examples, source/SVG
copying, sharing, and comparison. Share links serialize Mermaid source in the URL hash;
the source does not carry a separate authored theme graph. Copying SVG copies the rendered
markup, whose CSS variables may still depend on the host environment. These are useful
diagram-sharing mechanisms, but they do not replace Merman's self-contained design export.
[Playground page](https://github.com/obsidianmd/pele/blob/44da152bbb79b91f03e68016f42499bb25bf6e82/website/src/pages/playground.astro),
[Playground runtime](https://github.com/obsidianmd/pele/blob/44da152bbb79b91f03e68016f42499bb25bf6e82/website/src/scripts/playground.ts),
[Share codec](https://github.com/obsidianmd/pele/blob/44da152bbb79b91f03e68016f42499bb25bf6e82/website/src/lib/playground-link.ts)

The source/output divider supports pointer dragging, keyboard increments, Home/End,
double-click equalization, persistent proportions, minimum sizes, and live accessible
values. This is a concrete interaction checklist for Merman's existing resizable workspace.
[Column interaction](https://github.com/obsidianmd/pele/blob/44da152bbb79b91f03e68016f42499bb25bf6e82/website/src/scripts/playground-columns.ts)

## Transfer to Merman

Merman already authors separate canvas, surface, alternate surface, text, subtle text, border,
line, and accent roles, as well as category palettes. Some complex presets intentionally alias
several roles to the same shared color. The current friction is presenting many core and
family-detail values with equal prominence, not an absence of semantic roles. Start by grouping
and navigating the existing data; do not infer or add another required binding layer.
[Preset authoring](../../crates/merman-render/src/diagram_theme/presets/catalog.rs)

The current designer groups tokens by their authored group or value type. It displays a
reference-use count, but does not list those consumers or navigate to them. The inspection
DTO already carries token IDs, authoring paths, and consumer domains; a consumer browser
can build on that data without inventing a second editable source of truth.
[Designer](../../playground/src/components/theme-workbench/ThemeDesignEditor.tsx),
[inspection types](../../crates/merman-theme-contract/src/design.rs)

Reference inspection describes declared consumers, not the winning rule on each rendered
object. Consumer navigation must not relabel reference counts as applied-object counts.
Use existing execution evidence for supported explanations, and preserve uncertainty where
per-field source precedence is not available.

| Decision | Recommended application | Boundary |
| --- | --- | --- |
| Small semantic entry point | Start with recognizable shared colors, typography, palette, and effect values from an ordinary preset/starter document. | Do not require authors to create a token library before changing a node fill. |
| Global values plus local styling | Show which objects consume a shared value; offer navigation, rebind, and local copy at those consumers. | Preserve explicit references; equal colors are not evidence of a relationship. |
| Host design-system adaptation | Document how a frontend maps its own resolved colors/fonts into an ordinary Merman design. | Resolve CSS externally; do not add `var()`, arbitrary expressions, or DOM inheritance to Rust. |
| Semantic series colors | Present category palettes independently from critical/status colors and canvas controls. | Preserve existing target-specific palettes and ordering; eight colors is Pele's choice, not a Merman limit. |
| Clear cost boundaries | Explain that font/size changes can affect measurement and that exported artifacts contain admitted concrete styling. | Do not promise browser-only CSS recoloring for native PNG/PDF or reuse an old layout after a font change. |
| Local exceptions | Label fixed source styles distinctly so users understand why a global color did not affect an object. | Merman's existing source-style precedence remains authoritative. |

### Priorities for the designer

1. **Make a preset copy immediately understandable.** Present its existing labeled/grouped
   shared values before the complete semantic-target list. Use consistent role labels in
   maintained starter documents. Keep theme-specific values such as a neon glow or paper
   grid discoverable through their supported types and metadata. Existing labels/groups and
   explicit bindings can support this; another mandatory role schema is not justified by Pele.
2. **Explain the effect of an edit.** Expand the current usage count into a consumer list with
   human-readable object/part labels and navigation. Show bound, direct, cleared, and source-fixed
   values distinctly. This addresses author confidence more directly than exposing more fields.
3. **Separate canvas from the viewing surface.** Give exported canvas paint and preview backing
   different labels and previews. Explain text-on-series contrast separately. Avoid a single
   global “background” control that also changes unrelated foreground semantics.
4. **Keep advanced freedom.** Shared gradients, strokes, shadow stacks, and scoped rules remain
   ordinary design data. Pele does not provide evidence for removing those capabilities or
   replacing them with its smaller CSS variable vocabulary.

These are authoring and presentation recommendations, not findings that Merman's materializer
is broken. The current design already addresses retained references, typed shared composites,
preset copies, local detachment, source isolation, and portable exchange. The remaining user
friction should first be tested through the simpler editing flow above.

## Do not transfer these assumptions

Pele explicitly uses its own layout/appearance and ignores Mermaid themes and `themeVariables`
colors. Merman's Mermaid parity route must remain separately faithful; Pele's color vocabulary
cannot become the authority for Mermaid-compatible output.
[Compatibility](https://pele.run/compatibility)

CSS inheritance, selectors, browser media queries, and external stylesheets are runtime
facilities of Pele's embedding environment. Merman must also render without a DOM and export
native artifacts, so a portable authoring document must carry resolved, admitted meaning.
Similarly, Pele's source-style allowlist does not establish support for arbitrary shadow,
filter, spread, inset, or backdrop effects. A new drawing operation still needs implementation
and execution evidence in Merman's actual output paths.

No core API or serialization rewrite follows from this comparison. In particular, the
unreleased `ThemeRecipeV1`/`ThemeDesignV1` should remain version 1; improve ordinary starter
documents and editor behavior within that model before adding another abstraction.
