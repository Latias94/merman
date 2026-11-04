# Create, Customize, and Share Diagram Themes

This guide targets `0.10.0-alpha.1` and its experimental version-1 theme authoring
contract. Use a matching runtime artifact that exposes theme authoring and catalog operations;
a package version alone does not establish channel availability or feature support. Examples
assume an initialized SVG-capable runtime.

## Start with a theme

Choose the kind of change you want:

| Choice | Playground | CLI | Web options | Scope |
| --- | --- | --- | --- | --- |
| Mermaid's base appearance | **Appearance**, Mermaid collection | `--theme dark` | `site_config: {theme: "dark"}` | Shared Mermaid configuration; affects both engines in Compare |
| A Merman visual design | **Appearance**, built-in collection | `--theme-preset ghibli` | `theme: {preset: "ghibli"}` | Merman recipe: colors, canvas, typography and supported family effects |
| Your own Merman design | **Import JSON**, validate and adopt | `--theme-file theme.json` | `theme: recipe` | The same editable recipe, independent of the installed catalog |

These inputs can be combined. The Mermaid base supplies defaults, and a Merman recipe supplies
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
console.log(preset.family_designs, preset.application_notes);
const { svg, metadata } = renderSvgResult(source, {
  version: 3,
  theme: { preset: preset.id },
});
// Keep metadata with this SVG when explaining any unapplied styling.
```

The installed runtime's catalog is authoritative. Preview colors are illustrative recipe
colors; they are not a rendering or an output qualification. Hosts should show unknown
application notes using their readable message and offer configuration only as an explicit
user action.

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

Open **Appearance** to keep the panel beside the current diagram. One collection offers Mermaid
native themes, runtime-discovered Merman presets, standalone explorations, and the current imported
recipe. Search by name or source. Choosing a valid theme updates the workspace immediately;
closing the panel keeps that choice. **Undo appearance** reverses selections and font changes
without discarding source edits. Selecting a Mermaid native theme removes the Merman extension;
Advanced retains explicit composition of a base theme and an extension.

On narrow screens, the panel reserves space for the diagram preview. Drag the separator, or focus
it and use the arrow keys, to adjust the panel width on desktop or height in a stacked layout.
These two local display preferences are remembered separately.

### Standalone explorations and example scenes

Atlas (technical light), Nightfall (quiet dark), Letterpress (warm editorial), and Grove (natural
green) are standalone exploration recipes. Selecting one uses that recipe on the current source,
base theme, font, and configuration. They are not core preset IDs: use their downloaded recipe
with CLI `--theme-file`, Rust `compile_recipe`, or Web `theme: recipe`. Their names and source
labels are display metadata, not claims of runtime availability or family support.

The example viewer contains Flowchart, Sequence, Class, XY Chart, and Pie scenes with their source
and configuration. Its saved SVGs are fixed examples, not live previews of the current document.
The recipes, shared samples, configurations, and SVGs live in
[`playground/src/themes`](../../playground/src/themes/). The five example families have been
rendered with each recipe; other families and targets need their own review. Source styles and
explicit font/configuration choices can change the result.

To refresh a library SVG, render its saved source, configuration, and recipe together, for example:

```console
merman-cli render playground/src/themes/samples/xychart.mmd --config-file playground/src/themes/samples/xychart.config.json --theme-file playground/src/themes/recipes/atlas.theme.json --svg-id atlas-xychart --svg-pipeline parity --font-measurement deterministic --font-system off -o playground/src/themes/previews/atlas-xychart.svg
```

The Letterpress Class preview uses native measurement with Menlo to size its monospace signatures
(`--font-file /System/Library/Fonts/Menlo.ttc --font-system off` on macOS, replacing the deterministic
measurement flag). Fonts are not embedded. With another host font, review label bounds before
replacing the asset. Native SVG text does not imply native font measurement.

### Effective appearance and imports

Mermaid base themes affect both engines in Compare. Merman presets and recipes affect Merman only.
The pinned Mermaid catalog includes `default`, `base`, `dark`, `forest`, `neutral`, `neo`,
`neo-dark`, `redux`, `redux-dark`, `redux-color`, and `redux-dark-color`. `auto` is the Playground's
choice to omit an initialization override, not a Mermaid theme name. Source/configuration
declarations retain their normal precedence. The panel shows detected theme, font, and look
locations and offers navigation to their owner; selecting a theme does not silently remove them.

Unknown or unavailable saved preset IDs remain in the workspace and shared link. Playground omits
that extension for preview, keeps the Mermaid base and source styles, and shows a notice beside
the diagram and in image export. Core APIs still reject unknown preset IDs. Invalid recipes and
diagram errors retain their failure behavior; they do not trigger this fallback.

**Inherit from theme / source** adds no host font override. A named font is an explicit host
choice; returning to inheritance releases it. Historical links keep their original Trebuchet
default. Fonts resolve on the receiving host and are not downloaded or embedded.

**Import JSON** reads a standalone version-1 recipe. The Rust compiler validates it before the
user adopts it. Invalid imports stay local to the import proposal; the current diagram, image
export, and sharing remain usable. A later selection, closed panel, or workspace replacement
invalidates pending import work. Files over 256 KiB are rejected before reading. A diagram error
does not make an otherwise valid recipe invalid.

Original authored JSON, formatting, and shared-value relationships are retained. Export a recipe,
edit it with the [theme design skill](#design-a-theme-with-an-agent) or another editor, then import
the revision. The panel does not offer a visual theme editor.

**Download JSON** exports the actual standalone design for a runtime preset, or the original
JSON for an imported/exploration recipe. No diagram render is required. A native Mermaid theme
has no Merman recipe to download. Advanced settings can remove Merman styling while retaining
the Mermaid base; this does not guarantee a transparent diagram.

### Adopt the current render in a project

**Use in a project** provides Rust, CLI, and Web adoption code and downloadable files from a
matching completed render. The handoff preserves configured source and initialization settings
separately. Playground's configuration editor is injected into source `init`; moving those values
to CLI `--config-file` or Web `site_config` can change precedence. The generated materials must
retain their original ownership. The dialog freezes its input so later editing does not mix two
scenes in one download.

Use the theme file alone to reuse its design on another diagram. Use the complete handoff to
carry source, settings, recipe, output pipeline, and recorded environment conditions together.
Host fonts and browser measurement callbacks remain application responsibilities. Matching
inputs do not promise identical pixels across different font installations or measurement modes.
An unavailable preset fallback cannot be presented as working core-preset adoption code. Recipes
must pass the current runtime compiler before independent adoption; a diagram error does not
prevent adopting a valid recipe. Generated Rust materials enable `math` for mathematical labels;
CLI consumers need the matching optional capability when the source contains formulas.

### Inspection, output, and sharing

Advanced inspection exposes read-only recipe JSON, runtime support information, and same-render
theme metadata. Catalog intent, mechanism support, and actual execution answer different
questions. Unknown evidence versions/statuses and an empty diagnostic list do not prove support.

**Export** owns image format, size, background, and compatibility information for its frozen
artifact. `parity`, `readable`, and `resvg-safe` select pipelines; selecting one does not prove
portability. Output-target results stay separate from whether theme styling was applied.

**Share → Copy workspace link** includes source, config, font, pipeline, and the selected recipe
or preset. Named recipes use the `s4` compressed envelope; unnamed recipes use `s3`, and preset-only
workspaces use `s2`. Existing links remain readable. Recipe JSON is bounded to 256 KiB before
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

**Tools** contains engine comparison, diagnostics, and benchmarking. Compare applies the Merman
extension only to Merman; Mermaid.js receives Mermaid source, configuration, and the shared base.
Inspect that difference separately from theme A/B comparisons. Advanced settings can remove only
Merman styling for a common-input comparison; undo restores the prior selection.

## Choose a recipe for a diagram

A theme has a shared base and optional family-scoped rules. Select one recipe, then apply its
rules for the current logical family. A theme can be designed for Class without being designed
for Sequence. Switching diagram families does not select a different preset automatically.

Read the loaded runtime's `themeCatalog().presets`. Keep these facts separate:

| Field or result | Meaning |
| --- | --- |
| `available` | This artifact can construct the preset |
| `family_designs` treatment `dedicated` | The catalog records a family-specific design |
| `family_designs` treatment `base_only` | The shared appearance, with necessary family adaptations |
| Missing family or unknown treatment | Unreviewed design scope; preserve the value conservatively |
| `preview_colors` | Representative recipe colors for discovery, not a rendered preview |
| `application_notes` | Readable family guidance and optional explicit configuration suggestions |
| `qualified_cells` | Evidence for named scenarios and targets, not every possible diagram |
| Actual render admission | What this input, output target, resources, and policy can deliver |

The current catalog advertises dedicated design scope for Brutalist, Spotless and Ghibli, but it does
not claim pixel parity with Modern Mermaid. A successful SVG render is not proof of readable dense
labels, complete effects, or PNG/PDF portability. Cyberpunk carries a layered canvas and scoped
effects with bounded Flowchart, Sequence and XY Chart SVG/PNG/PDF observations. Larger scenes may
exceed native export budgets. Brutalist now provides bold borders, ordinal accents and hard shape
shadows with unfiltered labels; Spotless provides a two-axis paper grid and family-scoped font weights
without filters. Its Flowchart and Sequence labels and XY Chart titles use the default letter
spacing; these targets do not support typed letter-spacing rules.
Ghibli adds a warm, textured canvas and a five-color muted Pie palette with typed title,
percentage and legend text sizes. Its fonts resolve on the receiving host. The paper texture
uses tiled gradients; this does not imply generic pattern or every effect primitive is supported.

Flowchart node, edge and label glow/shadow effects currently require `flowchart.look: classic`.
The default Neo look does not consume these effect bindings; selecting Cyberpunk alone does not
change the look. Both ELK and Dagre support the Classic effect path. For example, set
`{"flowchart":{"look":"classic","layout":"elk"}}` in Mermaid configuration to retain ELK
and request the demonstrated Flowchart effects. Color and typography support has its own target
coverage and should be inspected separately from effects.

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

Native CLI exposes the same policy and optional render report:

```text
merman-cli render diagram.mmd --theme-file theme.json --theme-report theme-result.json -o diagram.svg
merman-cli render diagram.mmd --theme-file theme.json --theme-portability require-portable -o diagram.svg
```

Best-effort remains the default. Theme diagnostics go to stderr; image stdout stays clean.
`--quiet` suppresses human feedback without suppressing an explicitly requested report. Reports
describe the rendering stage, not successful file publication. Some early input/compile/cancel
failures happen before a report can be written; check the command's exit status and keep reports
with the invocation that produced them. See the [CLI guide](../../crates/merman-cli/README.md)
for batch and report-path constraints.

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

`source_paths` are JSON Pointers relative to the named recipe payload, not to the outer options
object. A complete-spec location refers to its original mixed `styles` array. A `design`
location identifies `/styles/{target}`, `/rules/{index}/style`, `/palettes/{target}`, or
`/canvas/base`, rather than the materializer's expanded array position.
Generated defaults have `generated: true` without an invented input location. Built-in preset
selection and direct Rust typed specifications have no original JSON document; exporting and
reimporting a preset gives locations in that new `design` document.

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

`styles` establishes defaults in deterministic target order, followed by `rules` in authored order
and then `palettes`. Later matching rules override earlier values on the same target and facet.
A family filter limits matching; it does not add CSS specificity. Separate semantic targets such
as Node and NodeLabel require their own styles. Diagram-owned styles retain their defined priority.

Omitted patch fields leave ownership unchanged, `null` means Clear, and `"transparent"` explicitly
requests transparent paint. Clear restores family base behavior, rather than exposing a previous
matching rule. Canvas base is a paint rather than a patch field: use `"transparent"`, not `null`.
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

Source or site role-specific font-size configuration owns that size without suppressing
a theme's weight or fill. Clearing a role's size restores its configured baseline; clearing
its weight removes the explicit weight. Explicit default values such as 16px are still
requests. Generic `text` / `axis` typography, ordinal role typography, and unimplemented
properties retain residuals when they win on visible text. An absent or hidden role does
not certify a rule as applied. Query support for the selected family and inspect the render
report; the new axis target names do not imply support in other families.

## Save and distribute

Save the complete `ThemeRecipeV1` document as JSON. A fresh consumer imports it directly; it does
not need to extract a spec or call a preset by its old name:

```ts
const restored: unknown = JSON.parse(saved);
const svg = renderSvg(source, JSON.stringify({ theme: restored }));
```

Passing JSON through the public API validates it in Rust. A TypeScript type assertion alone does
not validate a downloaded document. Keep the serialized `schema_version` and `kind`; unknown
versions and ambiguous preset/spec/recipe combinations are rejected. Normal resource admission
still applies. Do not assume identical output across artifacts with different capabilities or fonts.

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
