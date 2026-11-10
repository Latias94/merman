# Create, Customize, and Share Diagram Themes

This guide describes the current development version-1 theme authoring contract, including
optional paired appearances. Use a matching runtime artifact that exposes theme authoring and
catalog operations;
a package version alone does not establish channel availability or feature support. Examples
assume an initialized SVG-capable runtime.

## Start with a theme

Choose the kind of change you want:

| Choice | Playground | CLI | Web options | Scope |
| --- | --- | --- | --- | --- |
| Mermaid's base appearance | **Appearance**, Mermaid collection | `--theme dark` | `site_config: {theme: "dark"}` | Shared Mermaid configuration; affects both engines in Compare |
| A Merman visual design | **Appearance**, built-in collection | `--theme-preset editor --theme-appearance dark` | `theme: {preset: "editor", appearance: "dark"}` | Merman recipe: colors, canvas, typography and supported family effects |
| Your own Merman design | **Import JSON**, validate and adopt | `--theme-file theme.json` | `theme: recipe` | The same editable recipe, independent of the installed catalog |

These inputs can be combined explicitly. The Mermaid base supplies defaults, and a Merman recipe supplies
its supported styling. Explicit diagram styles and configuration retain property-specific
precedence. Choosing a preset does not change the source, layout or look.

For ready-to-run examples, see the [six-theme gallery](theme-gallery/README.md), covering
Flowchart, Pie, Sequence, Class, and XY Chart. Brutalist, Spotless, Cyberpunk, and Ghibli use
visual design references from [Modern Mermaid by gotoailab](https://github.com/gotoailab/modern_mermaid);
the [gallery credits](theme-gallery/README.md#design-references-and-license) retain the source
revision and MIT notice.
Ghibli has a dedicated Pie design. Brutalist has dedicated Flowchart, Sequence and XY Chart
designs and a base palette adaptation for Class. The catalog records each preset's scope;
unlisted families are **not yet reviewed**. A dedicated design does not guarantee every shape,
effect or export format. Review the actual preview outcome for your diagram.

Paper Cut, Carbon Copy and Pop Art are also built-in presets (`paper-cut`, `carbon-copy`,
`pop-art`). They share their editable JSON with the Playground library. Each has dedicated
Flowchart, Sequence, Class, XY Chart, Pie, State, ER and Mindmap treatments; Carbon Copy also
includes Kanban. Choose the preset for direct use, or export its complete recipe to customize it:

```rust
use merman::diagram_theme::{DiagramThemeCompiler, ThemePreset};

let compiler = DiagramThemeCompiler::new();
let theme = compiler.compile_preset(ThemePreset::PaperCut)?;
let editable = compiler.export_preset(ThemePreset::PaperCut)?;
let json = serde_json::to_string_pretty(&editable)?;
```

The CLI accepts `--theme-preset paper-cut`. Web and Node render options accept
`theme: { preset: "paper-cut" }`; Typst accepts `theme-preset: "paper-cut"`. Use a runtime
built from matching source: the package version alone does not establish these new IDs.
Playground's **Customize** opens the selected preset's full JSON, with copy and download actions.
Applying edits creates a custom recipe. The original preset stays available, and generated project
examples use its ID until the user chooses an edited recipe.

Playground presents these sources in one theme collection. Select a theme to apply it immediately
to the current diagram; its name and source stay visible after selection and sharing. The selected
preset shows reviewed families and application notes. Advanced settings retain the independent
Mermaid base and Merman extension choices. Font selection has one owner in Appearance.
Recommendations change only their listed keys and preserve unrelated configuration; source
statements can still override them. **Undo appearance** restores earlier choices while preserving
later source/configuration edits. A theme file contains the recipe; **Use in a project** packages
adoption code and the inputs of the current render.

For CLI discovery, run `merman-cli capabilities` to see presets with their design scope and
settings guidance, or `capabilities --json` for the same structured metadata. In Web:

```ts
const preset = themeCatalog().presets.find((entry) => entry.id === "ghibli");
if (!preset?.available) throw new Error("Ghibli is unavailable in this runtime");
const appearance = preset.appearances.find(
  (entry) => entry.appearance === preset.default_appearance,
);
if (!appearance) throw new Error("The runtime did not provide the declared default");
console.log(appearance.family_designs, appearance.application_notes);
const { svg, metadata } = renderSvgResult(source, {
  version: 3,
  theme: { preset: preset.id, appearance: appearance.appearance },
});
// Keep metadata with this SVG when explaining any unapplied styling.
```

The installed runtime's catalog is authoritative. Preview colors are illustrative recipe
colors; they are not a rendering or an output qualification. Hosts should show unknown
application notes using their readable message and offer configuration only as an explicit
user action. Preset IDs are exact catalog identifiers in Rust, render selections and exports;
leading or trailing whitespace is rejected.

## Choose an appearance

Choose one design, then its appearance. Editor, Gruvbox, and Ayu provide Light and Dark; artistic
designs may provide just one. A theme's available appearances are authored choices, not automatic
inversions. A light paper design can remain light inside a dark application.

| Choice | Behavior |
| --- | --- |
| Light or Dark | Request that declared appearance explicitly. An unsupported choice fails. |
| Document default | Omit the selector and use `design.default_appearance`. |
| Follow app/editor | The host resolves its current appearance before calling Merman. It uses the declared default only when the host appearance is absent from the design. |
| Unclassified concrete design | With no declared appearances or default, render the concrete values without a selector. Do not infer an appearance from colors or names. |

The core `ThemeAppearance` type contains only Light and Dark. CLI and binding selectors accept
`light` or `dark`, with omission for a deterministic default; they do not accept `auto`, `system`,
or `follow`, and do not read the operating system's appearance. Follow must not hide an admission
failure in a declared branch.

The three JSON shapes have different purposes:

| Shape | Purpose |
| --- | --- |
| Raw `ThemeDesignV1` | Authoring payload: tokens, styles, rules, and optional appearances. Accepted by the raw materialization convenience. |
| `ThemeRecipeV1` | Portable file with `schema_version: 1`, `kind`, and `design` or `complete_spec`. Save and exchange this complete document. |
| Render-selection wrapper | Preset/recipe choice plus an optional concrete appearance, or a complete specification choice. This expresses how to use a document. |

`design.appearances`, when present, contains one or both named entries and requires
`design.default_appearance` to name an existing entry. Both fields belong inside `design`,
not beside the recipe's root version. A complete-spec snapshot is concrete and unclassified.
Use the [paired example](#add-light-and-dark-appearances) to build a design.

Render options select a declared appearance through a wrapper:

```ts
const svg = renderSvg(source, {
  version: 3,
  theme: { recipe, appearance: "dark" },
});
```

Passing `theme: recipe` directly uses its declared default. The render wrapper does not belong
in a downloaded theme file intended for ordinary document exchange. CLI `--theme-file` also
accepts wrappers for automation, while `theme inspect` checks portable recipe documents.
Do not pass a raw design as though it were a recipe or a complete render selection.

Validate the whole document before adopting it. Invalid inactive appearances reject imports,
inspection, selected materialization, compilation, and grouped preset export. An engine request
replaces `theme` as a complete value; omission inherits and `null` clears it. An appearance-only
request cannot recover the existing engine recipe. Changing appearances on a prepared source
requires preparing it again.

Plain-ID export preserves the authored scope: `exportThemePreset("editor")` returns the complete
pair; `exportThemePreset("editor-dark")` returns that single declared appearance. Explicitly
requesting Light from the concrete dark ID fails. Exporting the complete pair does not select
the preview's current appearance or discard the other one.

## Fonts across hosts

A theme's ordered `font_stack` names the preferred fonts and fallbacks. It does not load font
files. Review both the selected measurement mode and the output viewer when adopting a design
such as Riso: the first available face can change text width, wrapping and weight.

| Host or output | How to use the theme's fonts |
| --- | --- |
| Web browser | Load application-owned web fonts before a one-shot render, including the weights/styles in use. `document.fonts.load()` starts an unused face loading; `document.fonts.ready` only waits for loads already started. Use a browser measurement session for the display document. `mountDiagram` with browser measurement rerenders when its document's fonts finish loading. |
| CLI SVG | The default measurement is deterministic and does not measure the named fonts. Pass repeatable `--font-file` / `--font-dir` options for native measurement with fixed assets, or `--font-measurement native` for available system discovery. Explicit assets form a closed set unless `--font-system on` is selected. |
| CLI PNG/JPEG/PDF | Builds with system fonts use one native context for measurement and painting by default. Explicit font assets make the source set reproducible. A missing glyph can still yield best-effort bytes with rejected target admission; inspect warnings and the theme report. |
| Rust and C FFI | Use a native font context or a host measurement callback when accurate font geometry is required. Rust's ordinary native context allows per-request fallback; C's `engine_new_with_fonts` requires glyph coverage. Font acquisition is a constructor service, separate from theme JSON. |
| Other bindings and Typst | Inspect the installed binding's capabilities. Native library code does not automatically expose font-file loading in every wrapper. The current Node facade and Typst plugin use deterministic measurement; mobile hosts can supply their display-stack callback through the supported constructor. |
| Exported SVG | Text retains its family names and needs compatible fonts in the receiving viewer. Supplying native font files for layout does not embed them into SVG. Opening the SVG elsewhere or rasterizing it in a different host can select different faces. |

For cross-platform recipes, provide named alternatives and a final `sans-serif`, `serif` or
`monospace` fallback. The current native SVG dependency does not recognize standalone `system-ui`
as a generic family; it can fall through to its serif fallback. A stack such as
`["Inter", "Arial", "system-ui", "sans-serif"]` retains a usable native fallback without
promising the same UI face on every platform. Quoted source font names retain their literal
meaning; Merman does not rewrite them into generic aliases.

Theme sizes use pixels. Inherited Mermaid theme font sizes also accept absolute CSS lengths,
including `pt`, `pc`, `in`, `cm`, `mm` and `q`, using 96 CSS pixels per inch. For example,
`themeVariables.fontSize: "24pt"` supplies 32px to layout and retains its configuration priority
over a typed base size. Family-specific numeric settings keep their own configuration rules.
Pie title, percentage and legend sizes and Tree View label sizes also resolve absolute CSS
lengths for measurement. Equivalent `24pt` and `32px` values reserve the same text geometry.
Tree View resolves parent-relative sizes against an explicit SVG root context; values that
still need the receiving document or host CSS remain unverified for fonts and text paint.
Relative or inherited sizes under a zero-sized root do not certify visible text. Edges,
icons and highlight backgrounds retain their independent paint consumers. Ishikawa keeps Mermaid's
root-size spacing separate from final text size: its text bounds and label boxes use the
displayed font, while bone spacing and multiline steps retain the root configuration.
Its ordinary head label uses 14px and weight 600. Invalid shared sizes inherit the standard
16px ancestor; a valid `!important` shared size can also override the head's independent size.

Raw HTML labels can also contain relative CSS sizes: `em` follows the
parent font, while `rem` depends on the receiving document's root font. Class labels and notes,
Requirement, Kanban, State, Block, ER, Mindmap, and Usecase use the ordinary 16px document-root baseline for HTML
measurement. A page with a different root size or a standalone SVG document can resolve `rem` differently. Prefer
pixel or parent-relative sizes when the same source must travel between hosts.

Class notes apply `class.padding` to each side of the measured label in Dagre and ELK. An omitted
setting retains the note-specific default of 6px per side; Class bodies and namespaces retain their
own defaults.

Unbreakable words can expand an HTML label beyond its CSS wrap width. Requirement and Flowchart
ELK cluster labels retain wrapping for the surrounding words. Kanban preserves the expanded
label container and export canvas while keeping its configured column and card widths; very long
words can therefore extend beyond a card or into a neighboring column, as in Mermaid.
Flowchart's exported canvas also includes explicit pixel or numeric cluster widths and heights
applied by source styles; the rectangle retains its original attribute position.
Requirement preserves Mermaid's node-label positioning while including text that extends beyond
the frame in the export canvas, including large multiline stereotype labels.
Git Graph also includes large commit and tag text in the export canvas when the selected measurer
provides direct-text baseline bounds, including rotated labels. Measurement profiles without those
bounds retain their established approximation. Stacked tags share a background width while each
caption keeps its own horizontal center.
HTML wrapping also keeps browser break opportunities inside ASCII identifiers such as `DOC-12345`,
so a narrow label reserves the rows needed after the hyphen; a leading negative sign stays with
its number.

HTML emphasis also affects font selection: `strong` and `b` request a weight bolder than their
parent, including each nested level; `em`, `i`, `cite`, `dfn` and `var` request italic. `code`,
`kbd`, `samp` and `tt` use monospace. Supported explicit font declarations, including
`font-weight:initial` and `font-style:inherit`, override these defaults. Supply the corresponding
faces to the host when their measurements matter; a requested weight does not provide a font file.

Native HTML measurement and export share the same font-size keyword values with a 16px default
`medium`. A zero-sized inline run contributes no glyph width or ink; its parent line box and
visible siblings can still occupy space. A receiving browser's default font preferences can
change keyword sizes, so explicit lengths are preferable for reproducible exports.
Known zero-sized chart labels remain in the SVG but do not count as visible consumers of
theme fonts or text paint.
Event Modeling keeps Mermaid's fixed 16px layout measurement when display sizes change;
its prepared HTML determines whether text, rather than its surrounding shapes, consumes styling.
Architecture, Block, C4, ER, GitGraph, Journey, Kanban, Requirement, State, Timeline, Pie,
Treemap, Usecase and XY Chart preserve valid
inherited sizes below 1px and zero. The common SVG stylesheet also preserves these sizes;
labels with an independent positive size can remain visible below a zero-size ancestor.
Journey's tspan task size and Timeline's root-size layout offsets retain their
independent configuration roles. Hiding text does not remove independently styled nodes, label
backgrounds or connectors. Typed theme sizes must remain positive; use the owning Mermaid
configuration when a family supports hiding labels with zero.
The native SVG pipeline converts absolute CSS font sizes to pixels before painting, including
`q` lengths and the `xx-small` through `xxx-large` keywords. Absolute keywords use the same 16px
`medium` baseline as HTML measurement, independently of the parent size. Admitted literal zero
sizes also become `0px`, including units such as `0rem` that the rasterizer may not recognize.
Invalid negative literal sizes are ignored so text inherits its parent's size instead of
disappearing. Parity SVG keeps the authored spelling. Positive host-dependent lengths and CSS
expressions retain their existing measurement and portability limits.

Packet preserves the configured root size for relative role sizes such as `1em`; its absolute
role defaults and fixed field geometry remain independent. Invalid role declarations inherit
that root. A size whose visibility depends on host CSS or loses its numeric magnitude remains
unverified; it cannot certify a theme font or text paint as applied. Best-effort SVG retains
admitted CSS, while strict theme validation rejects unresolved requested styling.

Radar appends `px` to its axis and legend size options, following Mermaid. Use a number such as
`24`, not a CSS length string such as `"24pt"`. An invalid declaration inherits the root size;
labels inheriting a known positive root can consume a typed base size even without a title.

Cynefin also follows Mermaid's numeric role expressions. Use JSON numbers for
`themeVariables.cynefin.domainFontSize` and `itemFontSize`: `24` produces a 26px title, while
the string `"24"` produces 242px through Mermaid's string concatenation. Subtitle and transition
sizes subtract one from the item size. Zero stays hidden; a negative or otherwise invalid
final declaration inherits the prepared root instead of being clamped to 1px.
Cynefin's fixed role positions do not expand to fit larger fonts; inspect description and
transition-label bounds after increasing sizes.

XY Chart title and axis sizes accept absolute CSS lengths, including `24pt` and zero lengths.
Ignored values inherit the prepared root unless a supported theme rule supplies the size;
omitted options keep the role's default size. A `null` chart-title size keeps its 20px default,
while nested axis sizes set to `null` inherit. These SVG attributes do not accept declaration
syntax such as `24px !important` or `24px;`. Unresolved sizes preserve their CSS and retain
unverified theme evidence instead of treating an approximate measurement as proof of visible
text. Its legend retains its separate numeric layout contract, 1px minimum and configuration priority.

Wardley uses the same accepted explicit font for annotation measurement and SVG inheritance,
including when no custom typography is requested. Empty or invalid font attributes leave a
supported theme stack available. Its configurable axis and node-label sizes stay separate
from fixed 10px and 11px annotation roles; unknown receiving-host sizes retain residuals.

Railroad measures direct SVG text in all four grammar dialects. Empty, whitespace-only or
sanitized-empty literals reserve their box padding without an extra font-size height.

Timeline keeps the root `fontSize` layout offset separate from the displayed theme-variable
size. Rejected display sizes inherit the ordinary text ancestor; valid unknown host sizes
retain their CSS and unverified typography or text-paint evidence. Architecture also preserves
unknown inherited text sizes without changing its independent Cytoscape layout font.

C4 element font settings follow Mermaid's CSSOM assignment: numeric sizes use pixels, while
string sizes retain their CSS units. Empty or rejected element sizes expose the inherited
root; they do not restore the initialized 14px default. Element names, type/description rows,
boundary annotations and relationship labels keep their separate sizing rules.

Block label measurement follows its effective `classDef` typography, including the default class,
and direct node styles. Class rules and direct declarations retain their CSS priority rather than
using a separate font choice for measurement.

Native measurement rejects source text geometry that it cannot measure, such as explicit
`letter-spacing`, including declarations containing CSS comments or escaped property names.
Semicolons inside quoted font names remain part of the name rather than separate declarations.

Use representative Latin, CJK and symbol labels when the content needs them. A font's presence
does not imply glyph coverage, and a successful theme status does not certify either one. The
[host measurement guide](../bindings/HOST_TEXT_MEASUREMENT.md) describes callback preparation,
native contexts and per-request fallback evidence.

## Categorical colors and repeated entries

An ordinal palette is a finite, ordered list. A category after its last entry reuses the first
color; the renderer does not invent additional colors or combine data into an `Other` category.
Category identity, the family's ordering rules, and legend associations remain intact. A correct
cycle does not guarantee that a dense chart is easy to read.

Keep line-series inks, labeled category surfaces, and emphasis colors independent when they need
different contrast. A pale paper color can work behind dark text and disappear as a thin line.
A light label that works on a dark canvas can disappear on a bright sector. Check actual opacity
and background layers: Pie normally paints sectors at 0.7 opacity while its percentage text and
legend swatches remain separate opaque elements.

The generated native presets author a separate twelve-color `pie-series` palette and `pie-text` ink.
Titles and legend labels keep their canvas-facing roles. Ghibli uses muted botanical colors for
its sectors while retaining its paper and text treatment. Editing a Pie token does not recolor a
line chart or a Mindmap branch. Saved artistic recipes keep their own finite capacities: Reading,
Atlas, and Nightfall use eight categories; most others, including Paper Cut, Carbon Copy and
Pop Art, use six. Monograph's grayscale remains a limited categorical distinction.

Riso uses four chart inks and repeats from the fifth series. XY Chart follows Mermaid's bounded
legend layout: if the complete legend does not fit the reserved chart region, it is omitted.
Increasing the chart height can make a larger legend visible. A longer palette does not change
that layout rule or prevent small Pie percentage labels from becoming crowded.

These generated presets also keep opaque `category-fills` and `category-text` independent of Pie opacity
and chart strokes. Mindmap's primary root has its own fill/text pair; branch labels follow the
branch category rather than the node's visit order. Journey's `task-label`, Timeline's
`node-label`, and Git Graph's branch `cluster-label` address text on colored surfaces, while
titles, outside labels, Git tags, and commit labels retain their separate roles. Explicit
family scale and label colors still own their slots. These fallback adaptations do not expand
the preset catalog's declared design scope.

Ghibli uses six darker botanical chart inks for thin chart marks and Sankey nodes/links, separate
from its pale labeled surfaces. Native Treemap sections use the category palette without a
uniform body fill hiding it. Gantt excluded calendar bands and text over section overlays have
independent roles; task-state label colors retain their own priority.

The family's color domain can impose another ring before palette lookup:

| Family | Color domain |
| --- | --- |
| Pie | Original section-label order, including entries whose percentages round to `0%`; those entries retain their legend slot. Mermaid's default uses twelve `pieN` values. A typed `pie-slice` palette uses its own length. |
| XY Chart | Plot/series declaration order across bars and lines, independently of the number of x-axis values. |
| Mindmap | Eleven branch/depth section slots; the root has a separate paint owner. |
| Git Graph | Classic branch colors wrap through eight Mermaid slots; tag and commit labels have independent paint owners. Color-generated themes have their own bounded adapter conditions. |
| Kanban | Task occurrences wrap through twelve family slots before selecting an authored palette entry. |
| Timeline | Section/task slots follow the configured family color ring. |
| Journey | Section runs follow `journey.sectionFills`' ring; actors have a separate palette. |
| Radar | The typed palette adapter currently supports twelve indexed slots; a thirteenth dependent series retains an unsupported-palette result. |
| Sankey | Layout-node order; source/target/gradient links follow the effective node colors. Labels normally sit outside nodes. |
| Treemap | First-seen section or immediate-parent category; sibling leaves intentionally share a category color. Section and leaf opacity affect the final text background. |

Review the first repeat as well as ordinary samples. With `k + 1` visible Pie sectors and a
`k`-color cycle, the last and first sectors share a color and touch at the circular boundary.
Separators and labels help preserve boundaries, but do not create an unlimited supply of unique
category identities. Inspect long legends, small values, and both appearances before choosing a
recipe for more categories. Source-owned colors and opacity remain deliberate user overrides.

These custom-preset changes leave official Mermaid palette behavior intact. Alpha presets remain
subject to their declared family scope and [qualification](preset-qualification.md); attractive
samples, successful compilation, and shared fallback styling do not grant additional qualified
cells or automatically promote a saved recipe into the native catalog.

## Family configuration and compiled rules

Mermaid compatibility configuration remains a separate input from compiled style rules.
Explicit family values can be supplied through `site_config`, source frontmatter/directives,
or the recipe's `design.mermaid` or `complete_spec.mermaid` compatibility fields. They retain
their source precedence.
C4 element, boundary and relationship colors, `themeVariables.zenuml` colors and typography,
Wardley anchor/annotation colors, Gantt axis color, and Mindmap label backgrounds use their
family owners. Measurement and SVG paint consume the same prepared font values.

An explicit Mermaid theme in diagram source selects its materialized paint palette, including
derived colors. Existing palette values take precedence over compiled paint defaults; roles
absent from that palette retain their compiled fallback. Individual explicit variables from
source or site configuration keep their property-specific dependencies. Site theme defaults
and a recipe's own compatibility theme supply fallback palettes and do not suppress compiled
style rules. Typography, geometry, effects, and the host canvas retain their independent owners.

ER HTML labels, Block labels and Requirement node labels use a non-empty `themeVariables.nodeTextColor`,
falling back to `themeVariables.textColor`. Journey titles use a non-empty `journey.titleColor`,
otherwise inheriting the rendered text color. These fallbacks preserve the selected source or
explicit configuration's paint ownership; a site theme's default palette still yields to compiled
paint rules.

| Input | Ownership |
| --- | --- |
| Site/base Mermaid theme, including CLI `--theme` | Fallback palette; does not disable compiled recipe rules |
| Authored source theme in frontmatter/directives | Materialized source palette takes precedence over compiled paint for present roles |
| Explicit Mermaid variables or family configuration | Property-specific source/configuration ownership |
| Compiled recipe rules | Supported defaults for properties not owned by authored values |
| Recipe canvas | Independent paint; a Mermaid palette selection does not remove it |
| Playground inherited font | Adds no font override; recipe typography or engine defaults remain available |
| Explicit Playground font | Host initialization choice; authored source/config fonts can still take precedence |

Moving `theme: forest` from source into `site_config` is therefore a semantic change when a
Merman recipe is also selected. Keep the location as well as the value when reproducing an
appearance. Playground lists detected theme, font and look declarations with their source;
these hints describe authored settings, not an independent certificate of computed SVG styling.

Sequence's colored Redux palettes use participant and activation color arrays. With a compiled
paint rule, explicitly configured arrays retain their paint ownership; derived default arrays
yield to that property's authored scalar color. Without a Merman theme, Mermaid's original
array-versus-scalar behavior is preserved. For supported Neo participant rectangles, use an Actor
effect or effect Clear to control the body shadow; the raw `dropShadow` variable does not control
every participant glyph in the Mermaid baseline.

Partial source configuration also preserves Mermaid's initialized derived colors. To change an
XY Chart or Wardley backdrop from source, set `themeVariables.xyChart.backgroundColor` or
`themeVariables.wardley.backgroundColor` directly. Changing only the root `background` in source
does not necessarily recompute an already initialized family backdrop. Root canvas paint remains
separate from these diagram-owned backgrounds.

Default XY Chart also keeps its initialized text colors when only root `primaryTextColor`
changes. Set the corresponding `themeVariables.xyChart` role directly for predictable overrides.
During site initialization, an explicit XY color object derives its missing fields from root
colors; a partial source object merges into the already initialized theme. Preserve both the
configuration location and the nested fields when reproducing an appearance.

Treemap uses `themeVariables.cScaleN` for section and leaf fills and `cScalePeerN` for section
borders, including with Default. Visible labels normally use `cScaleLabelN`, which overrides
the `treemap.labelColor` and `valueColor` stylesheets. Transparent leaves can fall back to
`textColor` for readability, using the final body fill after a compiled Node rule or palette.
Compiled label paint follows that selected source; unused
stylesheet colors do not block it. Timeline's colored Redux palettes cycle through
`borderColorArray`; an empty array restores the theme's `mainBkg`/`nodeBorder` fallback.
Native Treemap font admission checks styles selected by rendered nodes. An unused `classDef`
does not require support for typography that never reaches a label.
Put Treemap stylesheet options under `themeVariables.treemap`, including `titleFontSize` and
`titleColor`. The corresponding root `treemap` spellings remain Merman fallbacks when those
theme variables are absent. Treemap's SVG class sizes accept unitless numbers such as `24`,
equivalent to `24px`, while negative lengths and invalid identifiers leave the generated size
in effect. `font-weight:initial` resets to normal before labels are fitted or shortened.
Leaf fitting follows Mermaid's integer read of the inline CSS size, so a class size of `24pt`
can become `24px` during fitting even though its initial measurement uses 32px. Prefer pixel
sizes when the fitted label should retain the authored length. Inherited root sizes and section
text keep their separate measurement rules.
Leaf size keywords such as `inherit` can make Mermaid generate an invalid value-label position.
Merman preserves the CSS and omits the invalid coordinate; requested inherited-font evidence
retains a residual for that value, and strict theme completion rejects it. Use numeric pixel
sizes for portable value placement.

Set `themeVariables.archGroupBorderColor` directly to change an Architecture group border.
Changing only `primaryBorderColor` does not recalculate an initialized group color.
With a compiled theme, Architecture group surfaces paint from ancestors to descendants before connections and services;
their labels and icons remain in front. Opaque or translucent Cluster fills therefore affect
the enclosing surface without covering its content.

Cynefin and Wardley also retain initialized family colors. Set the selected role directly,
such as `themeVariables.cynefin.textColor`, `themeVariables.cynefin.arrowColor`,
`themeVariables.wardley.axisTextColor` or `themeVariables.wardley.componentLabelColor`.
Root colors supply missing roles; an unused fallback does not suppress compiled paint.
Ownership follows the value the renderer actually accepts, so ignored configuration values
do not disable a supported theme override.

Venn derives a circle's default stroke and contrasting label color from its final fill,
including a compiled Node fill. Explicit source stroke or label colors and independent
compiled stroke or NodeLabel rules keep their own priority.
Intersection SVG labels and HTML data text select colors independently: clearing an SVG
fill can expose its inherited root fill, while an empty HTML color declaration inherits
the receiving text color. Ownership follows each selected declaration; a rejected value
does not claim an unused primary-color fallback.
Mermaid centers an intersection label and its nested data text independently, so using both
in a small intersection can overlap. Theme verification does not certify collision-free layout.

Fonts, sizes, Quadrant point radii and Tree View line widths follow the same rule. An ignored
value does not own the property, and an unused fallback does not own the selected value.
Font lists are admitted before normalization, so a rejected list cannot become valid by
dropping empty entries or hide an accepted lower-priority font. Quoted font names and safe
host-dependent values retain their distinct spelling and portability requirements.
Family configuration determines its accepted units, font-array handling and numeric bounds. Pie
title, percentage and legend sizes remain independent; C4 element sizes and ER relationship
label sizes also retain their own configuration rather than inheriting every base-size change.
Usecase accepts absolute lengths for inherited text, while `usecase.actorFontSize` and
`usecase.usecaseFontSize` retain their independent numeric settings.

Pie ignores invalid role-size CSS, including negative lengths and nonzero unitless values;
those roles inherit the root size during measurement and theme reporting. Quadrant Chart retains
its numeric role attributes and layout, while rejected negative sizes inherit the root font for
visible-text evidence. A zero inherited size does not certify fonts or text paint; an independent
positive role size can restore that role's visible consumers.
Numeric CSS outside the supported range retains unverified evidence rather than being treated as
an invalid declaration or a literal zero.

C4 role fonts can explicitly inherit the diagram's font family with `inherit` or `unset`,
including boundary and relationship text. Measurement resolves the inherited family before
layout; the SVG retains the authored family declaration. Role sizes and weights remain independent.
An empty role or zero-sized text does not prove that a theme font was used. A separately sized
boundary name can remain visible when the other boundary rows have zero size.

GitGraph commit and tag sizes accept absolute CSS lengths such as `24pt`, equivalent to
`32px`, as well as supported parent-relative sizes. Backgrounds and text use the same resolved
size. Native font measurement rejects an unresolved size only when a rendered role consumes it.
CSS expressions and document-dependent sizes retain unverified theme evidence; an estimated
layout size does not prove that the requested font or text paint is visible.

Architecture text reports follow the rendered Markdown words and sanitized icon content.
Empty text projections do not prove that a theme font or text paint was applied. Local HTML
font declarations and host-dependent styles remain separate from inherited theme typography.

Requirement source node sizes accept absolute CSS lengths, including `24pt` as 32px. Its
inherited base size and positive typed sizes use the same value for measurement and emitted CSS,
including values below 1px. Hidden node, relationship and title text does not prove that a
theme font or text color was applied. An explicitly sized HTML descendant can restore visible
text below a zero-size ancestor. Native measurement rejects source node sizes that this adapter
cannot resolve, such as relative lengths or CSS expressions.

Requirement and ER theme reports use the actual SVG or HTML label projection. A local font name
does not prove that the inherited theme font was used. Host classes, document-relative sizes and
other unresolved text styles retain unverified evidence even when layout has a fallback estimate.

Support is checked per property. For example, a Gantt Title or Text rule with visible consumers
can apply its fill while retaining an `unsupported-typography` diagnostic for its font size.
Combining those properties in one rule does not make the unsupported styling portable; strict
completion still rejects it. Title rules without a visible diagram or section title are not
applicable.

Gantt section titles preserve `gantt.sectionFontSize` CSS lengths, including `24pt`, `2em` and
literal zero. Task labels retain the numeric `gantt.fontSize` setting. An invalid negative task
size inherits the root font for measurement and drawing. Fixed-size axis labels, diagram titles
and vertical task labels keep their own sizes. Section classes outside Mermaid's four title-color
selectors do not prove that a title-color rule was applied. An unresolved section size keeps
unverified theme evidence and is rejected by native font measurement when a rendered section
title consumes it; unused section settings do not reject output.

Journey's legend and `fo` / `old` labels inherit the base font stack and size. Its `tspan`
labels retain `journey.taskFontSize` and `journey.taskFontFamily`; setting the task family to
the CSS keyword `inherit` or `unset` lets the base stack apply without replacing the task size.
Empty or invalid task families also inherit; a quoted font name such as `"inherit"` is a literal
name. Host-dependent task families retain unresolved evidence. Theme reports account for font
stack and size separately and do not certify empty labels as visible consumers. Title-only
Journey and Timeline diagrams also account for their inherited typography; a fixed bold title
does not prove that a requested base weight was applied. Weight and style requests remain
unsupported where the family cannot measure and emit them consistently.

ER Swimlane top-level lane titles follow `flowchart.htmlLabels`; nested groups retain the root
`htmlLabels` mode used by ordinary clusters. Their measurement and theme evidence follow those
separate owners, so a source font on an HTML title does not suppress an unrelated SVG title's
theme font.

C4 presets pair node surfaces with label colors; compiled edge, marker, boundary-label and
text colors reach their visible terminals. Filled boundaries paint behind their descendants. Gantt presets preserve normal, active,
done and critical task paints with paired inside-label colors; outside labels use the surrounding
surface. An unqualified custom task rule still applies to every task state. Wardley anchor
labels follow compiled text/node-label colors while annotations, arrow labels and evolved
labels retain their independent foreground/background pairs.

ZenUML prepares frame, participant, message, lifeline, note, group, activation and text styles
together. Its font preparation is shared by measurement and SVG paint. Explicit namespace
values retain per-property precedence. Support reports describe the implemented terminals;
unsupported facets still produce residuals. Background and text rules have separate owners;
a background-only rule does not infer label colors.

## Design a theme with an agent

Use the repository's [design-merman-theme skill](../../.agents/skills/design-merman-theme/SKILL.md)
to create or refine a theme from a visual brief. For example:

> Use $design-merman-theme to create a warm paper-and-ink theme with restrained orange accents
> and small hard shadows. Preview Flowchart and Sequence, then deliver editable JSON.

The skill starts from an exported preset or a small design, maintains explicit shared
relationships, and renders representative diagrams through the existing Rust engine. Its bundled
Node command exposes preset export, materialization, support queries, and SVG with metadata from
the same execution. A matching source artifact is required while this contract is unreleased;
see the skill's [runtime setup](../../.agents/skills/design-merman-theme/references/runtime.md).

The deliverable is an ordinary version-1 recipe, preview SVGs with their sources and options, and
a short design note listing tested families, output target, and limitations. The recipe remains
usable without the skill. Runtime validation and support diagnostics do not replace visual review.

Import the resulting recipe into Playground to inspect and apply it, or pass it to CLI
`--theme-file`. Playground handles selection, import, preview, and distribution. Create or adjust
the recipe with the skill or an external JSON editor, then import it to inspect the result.

## Select, preview, and share in the Playground

Open **Appearance** to keep the panel beside the current diagram. The collection distinguishes
**Source / default**, Mermaid.js official themes, Merman built-in presets, theme examples, and
your imported design. Source/default omits a Mermaid initialization override; it is not Follow
app or an official theme. A paired design has one identity and an appearance control.
Presets and JSON examples sort by their displayed names in the current language.
Search spans all groups by name, ID, source, or group description and hides empty groups.
Choosing a valid theme updates the workspace immediately;
closing the panel keeps that choice. **Undo appearance** reverses selections and font changes
without discarding source edits. Selecting a Mermaid native theme removes the Merman extension;
an ordinary Merman selection releases a separate host Mermaid override to Source/default so the
design can supply its own fallback. Advanced retains an explicitly selected composition of the
Mermaid base and Merman design. Neither action silently removes authored source or configuration.

Follow app rerenders when the application's appearance resolves to a different concrete choice.
An explicit Light or Dark choice stays pinned. A host-mode change does not create an authoring
undo entry or invert a single-appearance canvas. The panel shows the supported and effective
appearance; switching designs must not leave an invisible unsupported choice.

On narrow screens, the panel reserves space for the diagram preview. Drag the separator, or focus
it and use the arrow keys, to adjust the panel width on desktop or height in a stacked layout.
These two local display preferences are remembered separately.

### Standalone explorations and example scenes

Open **Theme designs** from the toolbar or the Appearance panel to browse standalone
recipes. Reading and Prism provide coordinated Light/Dark examples. Riso, Pop Art, Metro, Carbon Copy
and Signal have distinct artistic treatments, alongside Field Notes,
Lacquer, Paper Cut, Aurora, Phosphor, Letterpress, Grove, Atlas, Nightfall, Blueprint, Porcelain, Chalk and Monograph.
The first visit uses the detected current diagram as a visible family filter in the search area.
Change the family selector or remove its chip to browse other diagrams. Search text, appearance
and style filters combine; **Clear filters** keeps the search text. Returning from detail or
reopening the library preserves explicit filter choices and browsing position. Closing and
reopening a detail also retains preview controls, adoption target and selected project file.

Cards combine a large primary diagram with two supporting previews, each labeled by type.
A family filter makes that sample primary across designs for direct comparison while retaining
the other previews. Without a family filter, each card uses its representative example where
declared, such as Metro's Git Graph or Blueprint's Architecture diagram. Opening a design defaults
to **Diagram gallery**, a responsive gallery showing that design's reviewed families together.
All designs include Flowchart, Sequence, Class, XY Chart, Pie, State, ER and Mindmap. Metro adds
Git Graph, Signal adds Gantt, Carbon Copy adds Kanban, Field Notes adds Timeline, and Blueprint
adds Architecture. Prism adds Radar, Quadrant Chart, Sankey and Treemap; Monograph adds Requirement
and Tree View. Entering from a family filter highlights its tile while keeping the design's
other examples available. Click a diagram to inspect it in an expanded preview with zoom, fit and source/configuration
controls. The expanded preview's family selector and **Previous example** / **Next example** buttons
switch examples; **Close expanded preview** returns focus and scroll position to the tile you opened.
**My current diagram** retains an inline preview and controls, rendering an isolated candidate
using the current source and configuration. In this inline preview, scrolling the mouse wheel
continues to scroll the detail page; hold Ctrl or Command while scrolling to zoom the diagram.
Drag with the mouse to pan. A one-finger touch gesture continues to scroll the page. In the
expanded preview, the mouse wheel zooms directly; drag to pan, or use one finger to pan and
two fingers to pinch-zoom on a touch screen. **Zoom in** and **Zoom out** also work in both views.
**Fit** scales the diagram to the available space and recenters it, so the fitted percentage
depends on the diagram and viewport. **Reset view** restores 100% and centers the diagram.
Showing source and returning to the inline diagram preserves its zoom and position.
Browsing does not change the workspace. **Apply to my diagram**
uses the selected design on the current source and configuration, with an explicit appearance choice.
Paper Cut, Carbon Copy and Pop Art select their built-in IDs when the runtime catalog provides them.
Their project examples use the preset directly; the JSON tab still exposes the complete editable
configuration. Other designs and edited recipes use CLI `--theme-file`, Rust `compile_document`,
or Web `theme: {recipe, appearance}`. Names and source labels remain display metadata; use the
runtime catalog and render evidence for availability and family support.

Use a direct link such as [Riso](https://frankorz.com/merman/?theme-design=riso) to open a design's preview and integration examples without changing the current workspace. Theme library adoption requires Merman v0.10.0-alpha.2 or newer.

The inputs have separate purposes:

| Material | Purpose | When to use it |
| --- | --- | --- |
| `theme.json` | Colors, typography, canvas and effects | Import it into Playground, or load it through one application integration. |
| Example Mermaid source and layout options | The content and arrangement of a gallery sample | Optional when adopting a theme for your own diagram. Generated example projects already apply these options. |
| Rust, CLI or Web code | Loads the theme and renders a diagram | Choose the integration for your application; these are alternatives. |

The gallery's layout options are ordinary Mermaid diagram configuration, not a second theme.
You do not need them to reuse the visual design. Generated `diagram.mmd` already contains those
options in an init directive; do not inject them again. Web `options.json` already contains the
theme. Inspect the generated files to see where each input is consumed.

Rust offers **Single file** and **Project files**. Single file shows and downloads only `main.rs`,
which embeds the raw recipe, diagram and initialization settings. Add `merman` (with `math`) and
`serde_json` to your Cargo dependencies and run the code without companion JSON or Mermaid files.
Project files shows a navigable file tree with the complete `Cargo.toml` and uses `include_str!` paths relative to
`src/main.rs`, with `theme.json`, `diagram.mmd` and `site-config.json` at the Cargo project root.
Downloading the example supplies the complete directory structure and setup instructions.

Each design includes syntax-highlighted theme JSON and Rust, CLI, and Web adoption code.
Inspecting a gallery tile selects its family for the integration code. Each tile also offers
**Use this example**, which selects that family and scrolls to the adoption section. The expanded
preview and adoption section provide family selectors limited to that design's reviewed examples;
the gallery keeps those examples visible.
The selected concrete appearance applies to every gallery tile, sample link and integration example.
Choose a tile or an adoption sample family before downloading its example project to include the
matching Mermaid source, theme, configuration, and setup instructions. These materials describe
an independent example, not a captured rendering of the current workspace. **Open sample** starts
a separate tab with the source and configuration, leaving the current work intact.

Standard gallery previews use their saved example scenes; current-diagram previews remain isolated from the workspace.
Fixed thumbnails, including Reading and Prism's separate Light/Dark assets, use saved SVGs and
lazy image loading. Expanding an example or previewing the current diagram uses the active runtime.
The shared samples, configurations, gallery recipes and SVGs live in
[`playground/src/themes`](../../playground/src/themes/). Paper Cut, Carbon Copy and Pop Art use
canonical recipes in [`crates/merman-render/assets/themes`](../../crates/merman-render/assets/themes/),
imported directly by the Playground and embedded by the core catalog. The sample inventory covers 19 families;
each catalog entry declares its own dedicated design scope. Monograph uses a grayscale design;
Prism keeps category identity across its two appearances. These design intentions do not certify
black-and-white printing, accessibility, or output portability. Other families and targets need
their own review. Source styles and
explicit font/configuration choices can change the result.
The shared diagram text is English in both interface languages; controls and explanations retain
English and Chinese localization.

Prism's Sankey treatment uses family-scoped cyclic Node paint rules to distinguish flows from
the darker text-bearing category surfaces used by other diagrams. With a compiled Merman theme,
Sankey draws flow bands below nodes and labels using normal compositing; bands keep their 0.5
opacity. Without a compiled theme, the renderer preserves Mermaid's multiply compositing and
original group order. Selecting an empty or canvas-only compiled theme also selects the themed
compositing path. Source `sankey.nodeColors` and `linkColor` retain their existing paint ownership.
Exact and cyclic rules apply only the supported solid, transparent, or cleared Node fill/stroke
facets; they do not add ordinal geometry, effects, or label typography support.

To refresh a library SVG, render its saved source, configuration, and recipe together, for example:

```console
merman-cli render playground/src/themes/samples/xychart.mmd --config-file playground/src/themes/samples/xychart.config.json --theme-file playground/src/themes/recipes/atlas.theme.json --svg-id atlas-xychart --svg-pipeline parity --font-measurement native --font-file /path/to/segoeui.ttf --font-file /path/to/seguisb.ttf --font-system off -o playground/src/themes/previews/atlas-xychart.svg
```

Class compartments and chart legends need actual font measurement. Browser previews use the
Playground's host measurer and available browser fonts. The categorical readability refresh uses
native measurement with explicitly loaded Windows font files for 54 affected saved SVGs; unchanged
examples retain their earlier exports. The Atlas example above uses Segoe UI regular and semibold.
For other recipes, supply checked files matching their actual font roles and weights with repeatable
`--font-file` arguments and `--font-system off`. Match the faces selected by the viewer, including
user-installed fonts such as Menlo, rather than assuming the first loaded fallback is displayed.
Record that measurement condition alongside the
same-render report. Fonts are not embedded; another host font requires a fresh label-bounds review.
Native SVG text alone does not establish native font measurement.

Riso's small opposing shadows suggest ink registration offsets on supported bodies; they do not
simulate physical ink mixing or paper noise. Phosphor scopes glow to connections, activations,
and data traces. Field Notes uses dark record cards, faint paper ruling, and lifted annotation
slips. Class and Pie use their own paint and typography treatments rather than unsupported body
effects. Their recipes remain editable without adding new preset IDs.
Blueprint combines a drafting grid, technical annotations and crisp outlines. Pop Art uses
poster colors, bold contours and offset ink. Lacquer pairs deep panels with asymmetric gold rims
and a restrained canvas sheen. Their dedicated treatments include the core scenes above.
Paper Cut uses independent hard paper edges plus soft contact depth. Aurora combines a gradient
night canvas, radial light ribbons and supported panel halos; text remains unfiltered.
Metro uses flat station signs and distinct route colors on a dark concourse. Its Sequence treatment
uses recessed navy station signs with yellow lettering and frames, cyan information slips and
lifelines, orange activations, and larger semibold message labels. These colors distinguish visual
roles rather than message status or actor identity. Carbon Copy uses violet side impressions,
typewritten records and red review marks. Signal uses charcoal equipment
plates, hard mounting edges and warning stripes confined to the canvas perimeter. Their Class
treatments rely on paint and typography rather than unsupported body effects.

State treatments distinguish ordinary states, composite headers, notes and transitions. ER
treatments use entity surfaces, alternating attribute rows and relationship paint; attribute text
shares the existing Text owner. Mindmap treatments preserve distinct branch colors through a Node
palette rather than flattening branches with a universal node fill. Its font stack and label paint
use their own supported routes; ER and Mindmap do not inherit Flowchart body effects.

Porcelain uses warm white ceramic surfaces, fine cobalt contours, terracotta annotations and a
bounded reflected edge on supported Flowchart/Sequence bodies. Chalk uses a matte slate ground,
cream contours and pastel categories, with contrasting dark labels on categorical surfaces.
Both include the same eight sample families and remain standalone editable recipes.

The Mindmap root uses the independent scalar `git0` compatibility value because the branch
palette excludes it. Redux Color ER relationship captions use `erEdgeLabelBackground` to keep
their backing readable; other Mermaid bases require their own review. These literals are separate
from retained palette/token bindings, so recoloring a palette does not update them automatically.
The current runtime reports `compatibility-unverified` and rejected target admission for these
fallbacks, even when the SVG renders and its typed family styling applies. The interface retains
that evidence; dedicated sample coverage is not an execution-verification or portability claim.

These designs also illustrate current authoring limits:

| Design request | Current behavior and supported alternative |
| --- | --- |
| Gradient Flowchart node bodies or Pie sectors | These role paths report `unsupported-paint`. The examples keep solid surfaces and place gradients in the canvas. |
| Class body glow or the same layered shadows as Flowchart | Class reports `unsupported-effect`. Its treatment uses paint and typography. |
| Match each ordinal body color with a separate label color | Flowchart supports ordinal Node fills, but its ordinal NodeLabel fill and Class ordinal Node/NodeLabel paint report `unsupported-paint`. Use body colors that work with the shared label paint, or explicitly style the corresponding source nodes. |
| Flowchart cluster-label weight or a local XY legend font stack | These role paths report `unsupported-typography`. Shared or family typography can still apply; support is property-specific. |
| Blur the scene behind a panel | Effects take the source graphic or previous effect result, not a backdrop input. Transparency and halos cannot reproduce backdrop blur. |
| Unlimited radial-gradient positioning | Percentage centers and radii have a validated range. Out-of-range values are rejected rather than silently clipped. |
| Add export margins with `canvas.bleed` | The field parses but currently reports `canvas / no-consumer` in the five example families. Use diagram layout options for spacing and inspect the rendered bounds. Signal's warning tape is a canvas layer whose thickness scales with viewport height; review it on unusually tall diagrams. |

Theme verification accounts for requested styling, not browser text bounds. During design,
deterministic XY legend reservations differed from actual Avenir and Menlo widths by roughly
0.6–4.2px. The shipped examples use typography checked in the browser, but receiving hosts and
font fallbacks still need layout review. Do not treat a verified theme as a pixel-fit guarantee.

### Effective appearance and imports

Mermaid base themes affect both engines in Compare. Merman presets and recipes affect Merman only.
The pinned Mermaid catalog includes `default`, `base`, `dark`, `forest`, `neutral`, `neo`,
`neo-dark`, `redux`, `redux-dark`, `redux-color`, and `redux-dark-color`. `auto` is the Playground's
historical wire value for Source/default, not a Mermaid theme name or Follow policy. Source/configuration
declarations retain their normal precedence. The panel shows detected theme, font, and look
locations and offers navigation to their owner; selecting a theme does not silently remove them.

Only three Mermaid designs are presented as pairs:

| Design | Light ID | Dark ID |
| --- | --- | --- |
| Neo | `neo` | `neo-dark` |
| Redux | `redux` | `redux-dark` |
| Redux Color | `redux-color` | `redux-dark-color` |

Default, Dark, Base, Forest, and Neutral stay independent. All literal IDs remain configuration
inputs, and Mermaid's `null` sentinel stays separate from registered themes. Hosts read these
groups from `mermaid_theme_designs`; they do not infer them from suffixes.
Mermaid `darkMode` and the recipe's compatibility `dark_mode` retain their own color-derivation
meaning. They do not select, classify, or generate Merman appearances.

Unknown or unavailable saved preset IDs remain in the workspace and shared link. Playground omits
that extension for preview, keeps the Mermaid base and source styles, and shows a notice beside
the diagram and in image export. Core APIs still reject unknown preset IDs. Invalid recipes and
diagram errors retain their failure behavior; they do not trigger this fallback.

**Inherit from theme / source** adds no host font override. A named font is an explicit host
choice; returning to inheritance releases it. Historical links keep their original Trebuchet
default. Fonts resolve on the receiving host and are not downloaded or embedded.

**Import JSON** reads a standalone version-1 recipe, including its complete optional pair.
The Rust compiler validates every declared appearance before the
user adopts it. Invalid imports stay local to the import proposal; the current diagram, image
export, and sharing remain usable. A later selection, closed panel, or workspace replacement
invalidates pending import work. Files over 256 KiB are rejected before reading. A diagram error
does not make an otherwise valid recipe invalid.

Original authored JSON, formatting, and shared-value relationships are retained.
**Customize** opens a highlighted JSON draft of the complete design, with navigation to shared
values and appearance overrides. Validate the whole document, then Apply it atomically.
Invalid or stale drafts preserve the active theme. This is a JSON workbench, not a graphical
token/rule editor. The [theme design skill](#design-a-theme-with-an-agent) or an external editor
can produce the same portable document.
Customize retains an explicit Mermaid/Merman composition and any still-supported appearance
preference. If the draft removes an explicitly selected appearance, choose a supported replacement
before applying it; a no-op edit does not silently change the composition.

**Download JSON** exports the actual standalone design for a runtime preset, or the original
JSON for an imported/exploration recipe, including inactive appearances. No diagram render is required. A native Mermaid theme
has no Merman recipe to download. Advanced settings can remove Merman styling while retaining
the Mermaid base; this does not guarantee a transparent diagram.

### Adopt the current render in a project

**Use in a project** provides Rust, CLI, and Web adoption code and downloadable files from a
matching completed render. The handoff preserves configured source and initialization settings
separately. Playground's configuration editor is injected into source `init`; moving those values
to CLI `--config-file` or Web `site_config` can change precedence. The generated materials must
retain their original ownership. The dialog freezes its input so later editing does not mix two
scenes in one download. Adoption code pins the captured concrete appearance, including when the
workspace used Follow; the downloaded theme document still retains its full pair.

Use the theme file alone to reuse its design on another diagram. Use the complete handoff to
carry source, settings, recipe, output pipeline, and recorded environment conditions together.
Host fonts and browser measurement callbacks remain application responsibilities. Matching
inputs do not promise identical pixels across different font installations or measurement modes.
An unavailable preset fallback cannot be presented as working core-preset adoption code. Recipes
must pass the current runtime compiler before independent adoption; a diagram error does not
prevent adopting a valid recipe. Generated Rust materials enable `math` for mathematical labels;
CLI consumers need the matching optional capability when the source contains formulas.

### Inspection, output, and sharing

Advanced inspection exposes recipe JSON, runtime support information, and same-render
theme metadata. Customize edits a complete draft separately from the selected concrete result.
Catalog intent, mechanism support, and actual execution answer different
questions. Unknown evidence versions/statuses and an empty diagnostic list do not prove support.

**Export** owns image format, size, background, and compatibility information for its frozen
artifact. `parity`, `readable`, and `resvg-safe` select pipelines; selecting one does not prove
portability. Output-target results stay separate from whether theme styling was applied.

**Share → Copy workspace link** includes source, config, font, pipeline, the selected recipe
or preset, and the appearance preference. Preference/provenance-bearing links use the `s5`
envelope, so Follow resolves on the receiving host. Other workspaces can retain the existing
`s2`/`s3`/`s4` forms. Historical links keep their original defaults and meaning of `auto`;
they are not reinterpreted as Follow. Recipe JSON is bounded to 256 KiB before
compression, with separate limits on the complete URL. Links reference the receiving runtime;
they do not pin an engine revision or embed fonts. Choices apply to the in-memory workspace;
save and reopen a workspace link to restore it after reload.

Image actions wait for the corresponding current render. An already-open image export stays bound
to its frozen artifact even when the workspace changes. Source and ASCII actions remain separate;
visual recipes do not style ASCII. External Mermaid Live and Markdown image services re-render
Mermaid inputs without Merman recipes; their actions identify that boundary before use.

### Theme background, preview backing, and Compare

Canvas paint belongs to the theme and exported image. Its base supports omission, explicit
transparency, a color, or a gradient. Omission preserves diagram behavior and does not imply
transparency. A transparent base retains decorative grids/gradients. Remove all theme canvas
paint by setting the base to `"transparent"` and removing its layers; diagram-owned backgrounds
may still remain. Complete-spec import does not infer shared relationships from equal colors.

Canvas layers retain paint order. Recipes can specify opacity, blend mode, offsets, linear/radial
gradients, and repetition. Serialization alone does not establish output support; inspect the
actual result and diagnostics. See [Add shadows and layered backgrounds](#add-shadows-and-layered-backgrounds).

**Preview backing** offers Automatic, Light, Dark, and Transparency grid in Visual and Compare.
It is a display preference, shared by both Compare panes. It does not change theme JSON, trigger
a render, or add image paint. Authored backgrounds remain visible. Automatic uses Mermaid
appearance, so a dark recipe can differ from its backing.

**Compare** is available beside **SVG** and **ASCII** in the preview tabs and from **Tools**.
**Tools** also contains diagnostics and benchmarking. Compare applies the Merman extension only
to Merman; Mermaid.js receives Mermaid source, configuration, and the shared base.
Inspect that difference separately from theme A/B comparisons. Advanced settings can remove only
Merman styling for a common-input comparison; undo restores the prior selection.

## Choose a recipe for a diagram

A theme has a shared base and optional family-scoped rules. Select one recipe, then apply its
rules for the current logical family. A theme can be designed for Class without being designed
for Sequence. Switching diagram families does not select a different preset automatically.

Read the loaded runtime's `themeCatalog().presets`. Keep these facts separate:

| Field or result | Meaning |
| --- | --- |
| Preset `available` | The compiler admits the complete exported design, including every appearance |
| `supported_appearances`, `default_appearance` | Declared choices and deterministic default |
| `appearances[].concrete_preset_id` | Exact ID for a fixed appearance; no suffix inference |
| Appearance `family_designs` treatment `dedicated` | The catalog records a family-specific design |
| Appearance `family_designs` treatment `base_only` | The shared styling, with necessary family adaptations |
| Missing family or unknown treatment | Unreviewed design scope; preserve the value conservatively |
| Appearance `preview_colors` | Representative recipe colors for discovery, not a rendered preview |
| Appearance `application_notes` | Readable family guidance and optional explicit configuration suggestions |
| Appearance `qualified_cells` | Declarations for named scenarios and targets, not every possible diagram |
| Actual render admission | What this input, output target, resources, and policy can deliver |

The catalog uses `schema_version: 1` with one grouped shape. Its single recipe and authoring
schema-version fields describe the loaded interface; reduced artifacts can report unavailable
interfaces. Every child of a grouped design shares the complete-document availability gate.
One passing appearance cannot make an invalid pair available. Inspect the matching appearance's
swatches and family notes when explaining a selection.

The catalog records dedicated design scope for Brutalist, Spotless, Ghibli, Paper Cut, Carbon Copy
and Pop Art. It does not claim pixel parity with Modern Mermaid. A successful SVG render is not proof of readable dense
labels, complete effects, or PNG/PDF portability. Cyberpunk carries a layered canvas and scoped
effects with bounded Flowchart, Sequence and XY Chart SVG/PNG/PDF observations. Larger scenes may
exceed native export budgets. Brutalist now provides bold borders, ordinal accents and hard shape
shadows with unfiltered labels; Spotless provides a two-axis paper grid and family-scoped font weights
without filters. Its Flowchart and Sequence labels and XY Chart titles use the default letter
spacing; these targets do not support typed letter-spacing rules.
Ghibli adds a warm, textured canvas and a twelve-color muted Pie palette with typed title,
percentage and legend text sizes. Its fonts resolve on the receiving host. The paper texture
uses tiled gradients; this does not imply generic pattern or every effect primitive is supported.

Flowchart Classic and the default Neo look both apply supported node, edge and label glow/shadow
effects with ELK or Dagre. Node body effects cover rectangles, rounded rectangles, diamonds,
circles and double circles. Hand-drawn geometry and other shapes can leave requested effects
unapplied; inspect the current preview and execution diagnostics for those cases. Theme selection
preserves the authored look, layout, configuration and source. Source styles retain precedence,
including explicit filters. Color and typography support has its own target coverage and should
be inspected separately from effects.

Exporting a complete recipe does not qualify every scene.
`describeThemeSupport()` answers mechanism questions for a family and target; it does not inspect
and approve the visual design of a particular scene. Web `renderSvg()` returns only the SVG
string, and `svgPlanJson().ready` checks artifact capabilities rather than applied theme facets.
Use `renderSvgResult()` to retrieve `{ svg, metadata }` from one execution, including versioned
`metadata.theme_execution_evidence`. Its theme and target states are separate; unknown versions
or statuses must not grant portability. The outcome includes explanatory `diagnostics` for
unverified theme work. BestEffort can return a useful preview together with a rejected target
status; strict policy turns that failed admission into an operation error.

Set `environment.theme_portability` to `"require-portable"` for the existing native strict policy,
or `"best-effort"` for the default behavior. Strict rendering rejects a result that fails admission
with the usual structured render error. It does not pick a different theme. Host fonts and
browser-dependent output may fail this requirement even when some theme paints were applied.
`resvg-safe` selects a pipeline and is not a substitute for the explicit policy.

Native CLI exposes the same policy and optional render report. For a recipe declaring Dark:

```text
merman-cli render diagram.mmd --theme-file theme.json --theme-appearance dark --theme-report theme-result.json -o diagram.svg
merman-cli render diagram.mmd --theme-file theme.json --theme-appearance dark --theme-portability require-portable -o diagram.svg
```

Best-effort remains the default. Theme diagnostics go to stderr; image stdout stays clean.
`--quiet` suppresses human feedback without suppressing an explicitly requested report. Reports
describe the rendering stage, not successful file publication. Some early input/compile/cancel
failures happen before a report can be written; check the command's exit status and keep reports
with the invocation that produced them. See the [CLI guide](../../crates/merman-cli/README.md)
for batch and report-path constraints.
For a classified design, each CLI report result records `selected_appearance` as the compiler's
choice. `evidence.materialized_theme` remains the actual Mermaid palette ID. Neither field proves
that source styles or explicit compatibility variables produce a particular visual mode.

Catalog `supported_output_ids` names artifact formats, such as `svg`. Support queries instead
name rendering contexts: choose `standalone-svg` for an independent SVG file or `browser-svg`
for browser-hosted SVG. `png`, `jpeg` and `pdf` retain the same IDs. Do not forward an artifact's
`svg` identifier directly into `describeThemeSupport()` or infer one context from the other.

There is no automatic cross-preset fallback. Outside a dedicated design scope, retain the selected
recipe and explain whether it has base-only or unreviewed styling. A host can offer an explicit
alternative. Unknown preset IDs and unsupported input schema versions are input errors, not reasons
to silently substitute a default. An unsupported effect must remain visible in support/admission
handling; choosing another preset must not conceal it.

## Export glow and shadows to PDF

PDF export can retain supported glow and shadow effects through localized rasterization of
filtered regions while retaining other vector content. This can increase export time and PDF
size. Inspect the actual PDF, including labels, arrowheads and outward glow; a successful SVG
preview does not guarantee native export support for the same scene.

Native conversion has a finite aggregate filter-primitive ceiling. For example,
`max_total_svg_conversion_filter_primitives` reports a rejected scene's observed count and limit.
The current ceiling is 128; it is a backend guard, not a caller-overridable theme option.
Reducing image resolution does not reduce this primitive count.

Keep the error visible when the scene exceeds a limit. Let the user explicitly simplify the
recipe or choose another output target. Do not silently discard shadows, replace the theme or
retry with relaxed resource checks. PDF filter-image sampling has a separate reported pixel
budget; it does not remove filter-complexity limits. Font names still resolve through the host.

## Understand unapplied settings

Inspect `metadata.theme_execution_evidence.diagnostics` from `renderSvgResult()`. For example,
a Sequence Lifeline rule can apply its blue stroke while retaining an unsupported radius:

```json
{
  "code": "unsupported-geometry",
  "subject": "rule",
  "target": "lifeline",
  "source_document": "complete_spec",
  "source_paths": ["/styles/1"],
  "generated": false
}
```

`source_paths` identify authored locations rather than positions in an expanded specification.
A complete-spec location refers to its original mixed `styles` array. Recipe-design locations
include `/design/styles/{target}`, `/design/rules/{index}/style`, `/design/palettes/{target}`,
and `/design/canvas/base`. Local exceptions retain `/design/appearances/{appearance}/...`
locations. These pointers do not include an outer render-selection or Options JSON wrapper.
Raw-design materialization has its own payload-relative reference paths; complete-document
inspection additionally identifies the supplying `token_path`.
Generated defaults have `generated: true` without an invented input location. Preset pointers
describe its exported recipe shape, not a file supplied by the caller. Direct Rust specifications
can have no original JSON document.

A rule-level diagnostic does not mean every property failed. Its reason identifies a category;
`property` is present only when the report knows a particular property, such as base typography.
Some locations remain broad: complete-spec top-level effect bindings point to `/effects`, and
execution diagnostics do not trace a consumer back through every shared token. Materialization
errors separately identify invalid values and references at their authoring paths. Output mutation and incomplete
verification may be reported without a source path. Fully overridden rules should not be reported
as unapplied requests.

Diagnostics are explanations, not an alternative admission result. An empty list cannot certify
Portable; a missing list means the producer did not supply explanations. Preserve unknown codes,
subjects and source-document identifiers, and avoid navigating a pointer whose document you do
not recognize. This result API explains successful BestEffort executions; strict failures continue
through the structured operation-error path.

## Radius depends on the target and shape

`radius` is a geometry value whose meaning belongs to the selected family and target. For
example, a Quadrant `chart-series` rule changes point radius; a Flowchart or Swimlane `node`
rule sets an existing corner channel. It does not change the node's shape kind or turn a
Diamond into a rounded path.

For Flowchart/Swimlane Node rules, the current contract is:

| Terminal | Numeric radius outcome |
| --- | --- |
| Classic/Neo Process or RoundedRectangle | Applied when the winning value reaches the rectangle; zero means square corners |
| Classic/Neo Diamond polygon | NotApplicable for the radius facet; preserve polygon geometry and other requested paints |
| Hand-drawn or a writer without verified corner semantics | Residual; strict admission rejects it |

These are writer observations, not a rule that every missing `rx`/`ry` is harmless. A rectangle
whose writer fails to consume a selected radius still has a residual. In a mixed fill-plus-radius
rule, Diamond fill can apply without claiming rounded geometry. An unsupported sibling property
still prevents the rule from being fully applied. Source rx/ry evidence is checked independently.

Explicit source rx/ry or an owning Neo configuration supersedes the typed radius. Either source
axis currently supersedes the scalar typed setting; the other axis retains the existing shape/configuration or SVG
automatic-radius behavior. Flowchart Node `radius: null` (Clear) remains unsupported, including on Diamond. Other
families keep their own Clear behavior; for example, Quadrant point Clear restores its baseline.

Discovery for Flowchart/Swimlane Node radius remains `conditional`: the query does not include
individual shape occurrences. Use execution evidence for the actual input. Cyberpunk's exported
complete recipe contains an ordinary Flowchart Node `radius: 10` rule; import preserves this rule
without enumerating node ordinals or substituting a different preset. This does not qualify the
complete reference appearance.

## Start with a small theme

A `design` recipe supports direct values and optional typed shared values. Keep the version and
payload together so the same file can be rendered, saved, or passed to CLI `--theme-file`:

```ts
import { renderSvg, type ThemeRecipeV1 } from "@mermanjs/web";

const recipe: ThemeRecipeV1 = {
  schema_version: 1,
  kind: "design",
  design: {
    tokens: {
      brand: { type: "color", value: "#fb7185", label: "Brand", group: "Colors" },
    },
    styles: {
      node: { stroke: { paint: { kind: "solid", color: { ref: "brand" } } } },
      "node-label": { fill: "#f8fafc" },
    },
    canvas: { base: "#142535" },
    rules: [{ family: "class", target: "node", style: { fill: "#263e55" } }],
  },
};
const svg = renderSvg("classDiagram\nclass Account", { theme: recipe });
```

There is no compulsory palette vocabulary. Name values according to your design, provide a type,
and bind only the properties that should share them. A direct value remains independently editable.
References are exact local IDs, do not interpolate strings, and cannot cross declared value types.
The Rust materializer validates missing references, cycles, units, and consumer domains.
This example declares no appearances, so it renders as a concrete unclassified design without
an appearance selector.

## Add light and dark appearances

Keep common values in `design` and put only deliberate exceptions in
`design.appearances.light` and `.dark`. Save this complete document as `theme.json`:

```json
{
  "schema_version": 1,
  "kind": "design",
  "design": {
    "tokens": {
      "surface": { "type": "paint", "value": "#f8fafc" },
      "ink": { "type": "paint", "value": "#1e293b" },
      "accent": { "type": "paint", "value": "#2563eb" }
    },
    "styles": {
      "node": {
        "fill": { "ref": "surface" },
        "stroke": { "paint": { "ref": "accent" } }
      },
      "node-label": { "fill": { "ref": "ink" } }
    },
    "canvas": { "base": "#ffffff" },
    "appearances": {
      "light": {},
      "dark": {
        "tokens": {
          "surface": { "type": "paint", "value": "#1e293b" },
          "ink": { "type": "paint", "value": "#f1f5f9" },
          "accent": { "type": "paint", "value": "#60a5fa" }
        },
        "canvas": { "base": "#0f172a" }
      }
    },
    "default_appearance": "light"
  }
}
```

Both appearances use the same semantic rules and token identities. A shared edit affects its
declared consumers unless a local token or rule supplies an exception. Equal direct values are
still independent. This small Flowchart-oriented example does not promise a complete design
for every family or output.

Composition is typed, with these replacement rules:

| Local section | Behavior |
| --- | --- |
| `tokens` | Replace entries by exact ID; omitted entries inherit. |
| `styles` | Apply after shared defaults and shared ordered rules. |
| `rules` | Apply after local styles, preserving authored order. |
| `palettes` | Replace supplied target palettes; lower after the combined rules. |
| `canvas`, `typography` | Replace the whole supplied section; omission inherits. |
| `mermaid` | Merge its declared scalar settings and variable entries only. |

Arrays do not implicitly append. Existing style Clear semantics remain distinct from omission
and transparent paint. Assets and requirements belong to the shared design; an appearance
cannot override them. This is not arbitrary recursive JSON merge.

Within a base `typography` section, family styles inherit each omitted property from `default`.
An authored `null` removes that property's local override: a family property inherits the shared
default, while a default property contributes no override to the renderer's configuration.
It does not pin the renderer's built-in font stack or numeric defaults. Role-style typography
Clear continues to restore the resolved family base.

Rust's `TypographySpec::family_style(family)` returns an owned, resolved `TextStyle`, combining
the shared default and the family's explicitly authored properties. Callers that require a
reference can bind this result locally and borrow it. Unspecified properties remain unspecified
when neither level supplies them.

Recipe Mermaid compatibility normalizes `mermaid.dark_mode` and `mermaid.variables.darkMode`
into one slot within each layer before composition. Equal declarations in one layer agree;
conflicting declarations in the same layer are rejected. Either spelling in an appearance
overrides the shared value. Native Mermaid source and site configuration retain their existing
`darkMode` settings.

Kanban derives section and task palette colors from `themeVariables.darkMode`. Choosing the
Mermaid Dark palette does not itself enable this separate color-derivation input. An explicitly
configured root `darkMode` remains a fallback;
when both paths are authored, the theme-variable value takes priority. The initialized root
default does not override the selected theme's value.

The same file can be checked and selected without changing its declared default:

```sh
merman-cli theme inspect theme.json
merman-cli render diagram.mmd --theme-file theme.json --theme-appearance dark -o diagram.svg
merman-cli batch README.md --output-dir generated --theme-file theme.json --theme-appearance light
```

`--theme-appearance` requires a Merman file/preset selection and accepts no automatic mode.
For automation, a `--theme-file` may also contain
`{"preset":"editor","appearance":"dark"}` or a recipe wrapper. A selector in that file must agree
with the flag. A raw `--theme-design-file` does not accept the flag; use a portable recipe.
`theme inspect` accepts only portable recipe documents, including `complete_spec`, or `-` for
stdin. It emits schema-1 inspection JSON directly, without `--json`. Render wrappers belong to
the broader `--theme-file` input grammar.

For Web, resolve any host preference before constructing the selection:

```ts
import { renderSvg, type ThemeRecipeV1 } from "@mermanjs/web";

function renderDark(source: string, recipe: ThemeRecipeV1): string {
  return renderSvg(source, {
    version: 3,
    theme: { recipe, appearance: "dark" },
  });
}
```

For Rust, add SVG-enabled `merman` from the matching source/runtime version and `serde_json`.
Place `theme.json` at the Cargo project root next to `Cargo.toml`, and save this as `src/main.rs`:

```rust
use merman::diagram_theme::{DiagramThemeCompiler, ThemeAppearance, ThemeRecipeV1};
use merman::{OperationControl, RenderOutput, RenderRequest, RenderTarget, Renderer, SvgRequest};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let compiler = DiagramThemeCompiler::new();
    let raw = include_str!("../theme.json");
    compiler.check_design_json_input(raw.as_bytes())?;
    let recipe: ThemeRecipeV1 = serde_json::from_str(raw)?;
    let theme = compiler.compile_document(recipe, Some(ThemeAppearance::Dark))?;
    let output = Renderer::new().render(
        RenderRequest::new(
            "flowchart LR\nA[Read] --> B[Export]\n",
            RenderTarget::Svg(SvgRequest::default()),
            OperationControl::new(),
        )
        .with_theme(theme),
    )?;
    let RenderOutput::Svg(Some(svg)) = output else {
        return Err("expected SVG output".into());
    };
    std::fs::write("diagram.svg", svg.svg())?;
    Ok(())
}
```

The raw preflight runs before typed deserialization, preserving duplicate-key and allocation
checks. A typed Rust author can construct `ThemeRecipeV1` directly. Built-in selections use
`compiler.compile_preset_selection("editor", Some(ThemeAppearance::Dark))`.
The resulting compiled theme is reusable; host appearance is never consulted during rendering.

## Inspect and materialize the complete document

The existing three JSON operations remain the entry points:

| Operation | Input | Role |
| --- | --- | --- |
| `materialize-theme-json` | Raw design, or a schema-1 `inspect`/`materialize` request containing a recipe | Validate bindings and inspect all appearances, or obtain one concrete specification |
| `export-theme-preset-json` | Plain UTF-8 ID such as `editor` | Export the complete editable recipe; no JSON export wrapper |
| `describe-theme-support-json` | Schema-1 family/output/subject query | Describe mechanism support, not whether a particular design offers Light/Dark |

Web exposes these as `materializeTheme`, `exportThemePreset`, and `describeThemeSupport`:

```ts
import { exportThemePreset, materializeTheme } from "@mermanjs/web";

const recipe = exportThemePreset("editor");
const inspection = materializeTheme({
  schema_version: 1,
  action: "inspect",
  recipe,
});
const selected = materializeTheme({
  schema_version: 1,
  action: "materialize",
  recipe,
  appearance: "dark",
});
console.log(inspection.supported_appearances, inspection.default_appearance);
console.log(selected.effective_appearance, selected.references);
```

Inspect accepts no appearance selector and admits every declared composition. Its schema-1
result includes `recipe_schema_version`, supported/default appearances, and each composition's
references. Materialize performs that same full admission, then returns `spec`,
`effective_appearance`, and selected references. A reference's `path` identifies its consumer;
`token_path` identifies the actual shared or local definition, such as
`/design/appearances/dark/tokens/accent`. These are declared bindings, not winning rendered rules.

The raw-design materialization convenience returns its existing concrete/default result. It
does not replace full-pair exchange or document inspection. Authoring operations intentionally
ignore render `options.theme`: supply the recipe and appearance in the action input.
Do not submit a standalone recipe as if it were a raw design.

Raw-design materialization and inspection use the same complete compiler admission as document
actions, including semantic targets and effect graphs. Raw inputs retain payload-relative
diagnostic/reference paths and encoded-byte limits on the raw design, without a recipe envelope.
Rust compilation failures in editable designs use `ThemeDesignCompileError::Materialization`
to retain the authoring diagnostic and its source path. Failures in a `complete_spec` remain
`ThemeDesignCompileError::Compilation`; both variants retain their error source.

For downloaded JSON, preserve the original text when constructing the action request:

```ts
JSON.parse(themeJson);
const inspection = materializeTheme(
  `{"schema_version":1,"action":"inspect","recipe":${themeJson}}`,
);
```

The preliminary parse checks only that the input is one complete JSON value. Rust validates
the closed schema, duplicates, references, every appearance, and resource bounds from the original
text. A failed inspection must leave the active document unchanged.

Ordinary download saves `recipe`, not `selected.spec`. To intentionally distribute a fixed
current-appearance snapshot, wrap that spec in
`{schema_version: 1, kind: "complete_spec", complete_spec: selected.spec}`.
Reimporting that snapshot neither restores a pair nor infers shared bindings.

## Customize an existing preset

Built-in presets export editable `design` documents with their bindings intact. Copy one and change
its shared values; the gradient stops, shadows, and styles explicitly bound to those values follow.
No color-equality search or preset-specific recoloring code is needed.

```ts
import { exportThemePreset, renderSvg } from "@mermanjs/web";

const recipe = structuredClone(exportThemePreset("cyberpunk"));
if (recipe.kind !== "design") throw new Error("Expected an editable design");
recipe.design.tokens!["neon-cyan"]!.value = "#22d3ee";
const svg = renderSvg("flowchart LR\nA[Start] --> B[Done]", { theme: recipe });
const saved = JSON.stringify(recipe);
```

Token labels, descriptions, and groups organize editor controls without changing render identity.
To customize one consumer independently, replace its reference with a local copy of the authored
value. Keep nested references that should still follow other shared values. Imported `complete_spec`
files remain editable with precise concrete controls; lost authoring bindings are not inferred.
In a pair, shared edits preserve appearance-local exceptions. Edit an entry under
`design.appearances.dark.tokens` when it should change only Dark; replacing a consumer reference
with a direct value detaches that consumer, not every equal color in either appearance.

Shared `styles` establishes defaults in deterministic target order, followed by shared `rules`,
then local appearance styles and local rules. Palettes lower after that combined rule list.
Later matching rules override earlier values on the same target and facet.
A family filter limits matching; it does not add CSS specificity. Separate semantic targets such
as Node and NodeLabel require their own styles. Diagram-owned styles retain their defined priority.

Omitted patch fields leave ownership unchanged, `null` means Clear, and `"transparent"` explicitly
requests transparent paint. Clear restores family base behavior, rather than exposing a previous
matching rule. Supported Flowchart body-effect terminals distinguish omission from Clear:
omission retains the look's default shadow, while Clear explicitly suppresses that shadow.
An explicit theme effect replaces the look's default instead of implicitly stacking with it.
Source filter ownership still takes precedence. Canvas base is a paint rather than a patch field:
use `"transparent"`, not `null`.
An omitted canvas leaves background ownership with the ordinary renderer behavior.

## Add shadows and layered backgrounds

An effect value can expose an ordinary `shadow-stack`, whose dimensions use pixel lengths and
whose colors and opacity can be shared or local. `std_deviation` is the Gaussian standard deviation;
spread and inset shadows are not part of this stack.

```json
{
  "schema_version": 1,
  "kind": "design",
  "design": {
    "tokens": {
      "raised": {
        "type": "effect",
        "label": "Raised surface",
        "value": {
          "kind": "shadow-stack",
          "composition": "independent",
          "shadows": [
            { "offset_x": 5, "offset_y": 5, "std_deviation": 0, "color": "#111827" }
          ]
        }
      }
    },
    "styles": { "node": { "fill": "#ffffff", "effect": { "ref": "raised" } } }
  }
}
```

`independent` shadows each use the original shape, then merge below it in authored order.
`cascade` applies each shadow to the preceding result. Advanced effect graphs retain the concrete
primitive interface. Canvas paints support solid colors, gradients and patterns, with optional
ordered layers, opacity, offsets and blend modes. These values use the same generic binding controls.

Supported body effects in Flowchart (including its Agentflow/Swimlane consumers), State, Usecase
and rectangular Sequence actors paint depth behind connectors or lifelines. The body surface
and its text retain their foreground position; a translucent surface is emitted once. This
ordering preserves the visible connection outside a body's own contour without moving its
route or shortening the authored shadow. Source-owned filters and unsupported shapes/looks
retain their existing behavior. Label, message, activation and plotted-series effects have
separate semantic owners.

Body depth and surface are separate scene layers. Each retains the selected shape's paint
opacity, and the depth filter derives its mask from that shape's fill/stroke alpha. A translucent
surface can therefore reveal its depth. The effect's `color_space` governs the filter's internal
shadow computation; the final depth and surface composite normally in the SVG canvas. This alpha
design replaces the former single filtered-body composite, whose whole-object opacity and
linearRGB source merge can produce different overlap colors. Independent/cascade shadow order
and `Previous`/`SourceGraphic` inputs remain defined within the depth calculation.

State retains source-owned composite group operations. With HTML labels, a source structural
class can apply opacity or a filter to an entire composite body and title, or to a nested Dagre
root. Splitting that group between depth and foreground would change its compositing. When the
source requires such a group operation, State keeps the complete source hierarchy and reports
the requested body depth as `unsupported-effect`; BestEffort retains the source appearance,
while RequirePortable rejects the incomplete theme. Ordinary shape fill/stroke alpha and
native-label shape rules retain their supported depth paths. The group check includes the browser
aliases `-webkit-opacity` and `-webkit-filter`. Ordinary source body filters also recognize the
filter alias, using the last valid declaration while retaining its authored property spelling.

Saving a theme does not establish family or export support. Query support for the requested family,
output and mechanism, and inspect execution metadata on the actual render. Keep asset licensing
and host-font requirements with shared documents.

## Style XY Chart series

An XY Chart `chart-series` rule addresses the plotted geometry. Ordinals are one-based in
source declaration order across both bar and line series. For `bar, line, bar, line`, the last
line is ordinal 4. A cyclic selector keeps that same order; it is not tied to DOM children or
numbered independently by plot kind.

Set `"variant": "bar"` or `"variant": "line"` to address only that plot kind. Combine it
with an ordinal to intersect both conditions: `"variant": "bar", "ordinal": {"exact": 2}`
selects the second declared series only if it is a bar. Omitting the variant or selecting
`"default"` addresses both kinds. These rules merge property by property in author order;
a later default rule can override an earlier bar rule. Kind selectors currently have terminal
consumers only for XY Chart `chart-series`; their presence in discovery is not an all-family
support promise.

For example, this complete-spec rule styles the first series with a translucent red interior
and an opaque green border:

```json
{
  "kind": "rule",
  "family": "xychart",
  "target": "chart-series",
  "ordinal": { "exact": 1 },
  "style": {
    "fill": "#ff0000",
    "fill_opacity": 0.25,
    "stroke": { "paint": "#00ff00", "width": 4 }
  }
}
```

Solid and transparent fill/stroke, stroke width, and the three opacity channels apply to bar
and line geometry. `opacity` affects the whole mark; `fill_opacity` affects its fill, and
`stroke.opacity` affects its stroke. Visible legend markers follow the same series style.
Point labels, bar data labels and legend text keep their own paint; making a mark translucent
does not dim its labels. A line normally has no fill; an explicit fill paints the area closed
by its path. Do not use a line fill to change point-label text.

`null` clears the selected property back to the family baseline and blocks that property's
theme-palette fallback. Baseline bars use the configuration palette and have no border;
baseline lines have no fill and use a 2px stroke. Opacity clears to 1. An explicit source or
site `themeVariables.xyChart.plotColorPalette` owns the series colors, while theme widths and
opacity remain independently applicable. Unsupported winning properties, such as dash arrays, still retain residuals even when another
property in the same rule was applied.

Series effect rules and target bindings support ordered zero-spread drop shadows. A rule's
effect wins over a binding; explicit `null` removes that series effect. Filters attach to actual
marks and visible legend markers, leaving labels independent. Paint outsets expand the SVG
viewBox without changing the plot layout, including for horizontal or vertical flat lines.
Unsupported effect primitives retain residuals; SVG emission alone does not certify a native
export target.

The public Cyberpunk recipe uses these kind selectors and a three-color cycle: cyan
`#6cc6cb`, purple `#c77dff`, green `#7ce38b`. Bars have 20% fill opacity, a 2px border and an
8px shadow; lines have a 3px stroke and a 6px shadow. The cycle follows global series order
and repeats after the third series. Saving the preset includes these rules and effect graphs.
The recipe also scopes cyan (`#00f2ff`) text to the XY Chart title (18px, weight 700), axis
titles (13px), and legend (12px). Tick-label sizes retain their baseline. These values survive
preset export and fresh import. Scoped text glow has bounded SVG/PNG/PDF scene observations;
these do not qualify every input or the full Cyberpunk design. Public qualification cells remain empty.

## Style XY Chart text roles

Use `title`, `axis-title`, `axis-label`, and `legend` for independently sized text. The two
axis targets refer to logical axes, so their meaning survives horizontal orientation.
These targets accept solid/transparent fill and static `font_size_px` / `font_weight` rules,
with an omitted or `default` variant. For example:

```json
{
  "kind": "rule",
  "family": "xychart",
  "target": "axis-title",
  "style": {
    "fill": "#00f2ff",
    "typography": { "font_size_px": 13, "font_weight": 600 }
  }
}
```

Font sizes and weights participate in measurement before layout and in the final SVG.
`text` paint is inherited by these roles; axis text additionally inherits `axis` paint.
Properties merge in author order: a later `axis` fill can override an earlier `axis-title`
fill. Axis-title/axis-label styles do not paint axis lines or ticks. Legend text inherits
`text` paint independently of its marker, which uses the series style. Point labels retain
their separate paint behavior.

Valid source or site role-specific font-size configuration owns that size without suppressing
a theme's weight or fill. Ignored values leave a supported theme size override available.
Clearing a role's size restores its configured baseline; clearing
its weight removes the explicit weight. Explicit default values such as 16px are still
requests. Generic `text` / `axis` typography, ordinal role typography, and unimplemented
properties retain residuals when they win on visible text. An absent or hidden role does
not certify a rule as applied. Query support for the selected family and inspect the render
report; the new axis target names do not imply support in other families.

## Save and distribute

Save the complete `ThemeRecipeV1` document as JSON. A fresh consumer imports it directly; it does
not need to extract a spec or call a preset by its old name:

```ts
JSON.parse(saved); // Verify that saved contains one complete JSON value.
const optionsJson = `{"theme":${saved}}`; // Preserve the original recipe text.
const svg = renderSvg(source, optionsJson);
```

Use `renderSvg`'s raw-JSON options argument to pass the recipe unchanged to the Rust runtime.
`JSON.parse(saved)` only checks that the input is one complete JSON value; the runtime validates
the recipe schema and rejects duplicate keys. Do not reserialize the parsed recipe: doing so
silently discards duplicate keys before the runtime can reject them. A TypeScript type assertion
alone does not validate a downloaded document. Keep the serialized `schema_version` and `kind`;
unknown versions and ambiguous preset/spec/recipe combinations are rejected. Normal resource
admission still applies. Do not assume identical output across artifacts with different capabilities
or fonts.

A recipe's canvas is part of the exported image. Scaling the complete SVG or raster also scales
its grid, background and diagram. A host's separate screen-fixed background is not carried by a
recipe unless it has been explicitly represented in the canvas.

```sh
merman-cli render --theme-file my-theme.json --format svg diagram.mmd
```

For initial distribution, use `my-theme.json` plus a README and the relevant license/attribution
files. The README should identify the author/revision, intended families, tested outputs, resource
requirements, and known limitations. These are accompanying metadata, not extra recipe fields.
Modified exports do not inherit the built-in preset's catalog identity, design claims, or
qualification. Distribution, downloading, caching, and update policy belong to the host.

A saved recipe is not automatically self-contained. A font-stack name does not embed a font;
document the required host fonts. Caller-supplied font bytes in recipes are rejected. The asset,
alias and embedding schema remains experimental; its presence in exported types does not promise
an embedded-font backend. Do not assume a recipient has your host fonts or that text layout is
identical across hosts. Merman does not add a theme registry, remote loading, or automatic font
installation as part of this contract.

### Sequence control surfaces

Sequence control structures (`loop`, `alt`, `par`, `opt`, `break`, and `critical`)
have three independent theme targets:

- `loop`: frame lines and section separators; static/default stroke paint, width and
  bounded effects. Fill and rounded corners are not supported on these open lines.
- `loop-label-background`: the keyword polygon; static/default fill, stroke paint,
  width and bounded effects. The polygon does not become rounded from a radius property.
- `loop-label`: keyword, primary title and section title text.

Each target has its own winners and effect Clear semantics. Clearing the keyword background
filter does not clear the frame or text filter. Source-owned colors do not suppress an independent
width or effect. Request support discovery for the exact target/facet and inspect the final output
report; the presence of a known target does not establish native portability.

Before the alpha.7 public contract freeze, the earlier unpublished `loop` box-paint rules were
moved to `loop-label-background`. Current recipes and internal authors use the new target;
there is no compatibility alias or schema-version change. Historical migration evidence retains
its original projection names.
