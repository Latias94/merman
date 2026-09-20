# Create, Customize, and Share Diagram Themes

This guide describes the unpublished `v0.8.0-alpha.7` source contract. It does not claim that
published alpha.6 packages implement this API. Examples assume an initialized SVG-capable runtime.

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
| `qualified_cells` | Evidence for named scenarios and targets, not every possible diagram |
| Actual render admission | What this input, output target, resources, and policy can deliver |

The current catalog advertises no dedicated Modern Mermaid reproduction. A successful SVG
render is not proof of readable dense labels, complete effects, or PNG/PDF portability.
Cyberpunk now carries its layered canvas and declares scoped Flowchart/Sequence shape glow,
but those family effect consumers are still incomplete. Their residuals prevent qualification;
exporting the complete recipe does not make the current preset a finished reference reproduction.
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

There is no automatic cross-preset fallback. Outside a dedicated design scope, retain the selected
recipe and explain whether it has base-only or unreviewed styling. A host can offer an explicit
alternative. Unknown preset IDs and unsupported input schema versions are input errors, not reasons
to silently substitute a default. An unsupported effect must remain visible in support/admission
handling; choosing another preset must not conceal it.

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
object. They refer to the original mixed `styles` array even when authoring expansion inserts
additional rules. A `definition` diagnostic can instead identify explicitly authored token paths.
Generated defaults have `generated: true` without an invented input location. Built-in preset
selection and direct Rust typed specifications have no original JSON document; exporting and
reimporting a preset gives locations in that new `complete_spec` document.

A rule-level diagnostic does not mean every property failed. Its reason identifies a category;
`property` is present only when the report knows a particular property, such as base typography.
Some locations remain broad: top-level effect bindings point to `/effects`, and expanded
Definition typography currently has no precise token location. Output mutation and incomplete
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

For a palette and ordinary scoped rules, author a `definition` recipe. Keep the versions and
payload together so the same document can be rendered, saved, or passed to CLI `--theme-file`:

```ts
import { renderSvg, type ThemeRecipeV1 } from "@mermanjs/web";

const recipe: ThemeRecipeV1 = {
  schema_version: 1,
  kind: "definition",
  definition: {
    authoring_schema_version: 1,
    expansion_version: 1,
    tokens: {
      canvas: "#142535",
      surface: "#22354d",
      text: "#f8fafc",
      border: "#fb7185",
      line: "#94a3b8",
      series: ["#38bdf8", "#fb7185", "#a3e635"],
    },
    styles: [
      { kind: "rule", family: "class", target: "node", style: { fill: "#263e55" } },
    ],
  },
};
const svg = renderSvg("classDiagram\nclass Account", { theme: recipe });
```

Token expansion defines a shared base; it does not guarantee every target in every family consumes
each property. Effects, layered backgrounds, and asset-bearing recipes use `complete_spec`.
Both representations compile through the same theme engine.

## Customize an existing preset

Call `exportThemePreset(id)` and inspect `kind`. A complete recipe retains canvas layers, effects,
assets, typography, and scoped rules. Modify a copy and preserve fields unrelated to the change.
Do not reconstruct a complex recipe using only its palette.

The executable [Web example](../../platforms/web/examples/custom-theme.mjs) changes the canvas base,
Node border paint, and Class Node fill in a complete recipe. Its operations are equivalent to:

```ts
import { exportThemePreset, renderSvg } from "@mermanjs/web";

const recipe = structuredClone(exportThemePreset("cyberpunk"));
if (recipe.kind !== "complete_spec") throw new Error("Expected a complete recipe");
const spec = recipe.complete_spec;
spec.canvas = { ...spec.canvas, base: "#142535" };
spec.styles ??= [];
spec.styles.push(
  { kind: "rule", target: "node", style: { stroke: { paint: "#fb7185" } } },
  { kind: "rule", family: "class", target: "node", style: { fill: "#22354d" } },
);
const svg = renderSvg("classDiagram\nclass Account", { theme: recipe });
const saved = JSON.stringify(recipe);
```

For matching rules on the same target/facet, later entries override earlier entries. A family filter
restricts where a rule matches; it is not CSS specificity. Properties omitted from the patch retain
existing ownership. Separate semantic targets, such as Node and NodeLabel, need their own rules;
a generic Text rule is not a universal replacement for every specialized label. Diagram-owned
styles retain their defined priority over theme defaults.

For patchable style fields, omitted means no override, `null` means Clear, and `"transparent"`
means explicitly transparent paint. Clear restores the family's base behavior; it does not mean
transparent or "remove this last rule and reveal the previous theme rule." Canvas `base` is a paint,
not a patch field: use `"transparent"`, not `null`, for a transparent canvas.

Check the operation's theme execution metadata as well as its SVG. For example, Class Node fill
Clear currently restores the base paint but retains an `unsupported-paint` diagnostic and rejected
target admission. Explicit color and transparent fill do not remove the ordinary SVG target's
Unverified status. A visible color change alone does not establish export qualification.

This example changes explicit roles. It is **not** a global brand-color parameter. Cyberpunk
contains independent node, actor, edge, marker, and data-series roles. Replacing every
identical color string can damage those distinctions. A generated recipe is an editable snapshot,
not a reversible record of the preset's color roles.
For repeatable edits, keep a small creation function that exports a preset, applies your explicit
changes, and saves the result. Keep its parameters and library version with the creation code.
Re-running that function creates a new snapshot; it cannot automatically preserve manual edits
made only to an earlier JSON file.

Convenient semantic parameters for complete complex presets are not yet public. They must first
be validated against actual canvas, effects and family consumers. There is no global recoloring
API, symbolic token sidecar, or automatic reconstruction of creation parameters from imported JSON.

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
preset export and fresh import. Text glow and final public-scene validation remain open;
this is not qualification of the full Cyberpunk design. Public qualification cells remain empty.

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
include host font requirements, and state which font resources are actually carried by the recipe.
Preserve applicable attribution when redistributing assets. Do not assume a recipient has your host
fonts or that its artifact supports embedded fonts. Missing-resource and cross-platform font
journeys remain part of the pre-release audit. Merman does not add a theme registry, remote loading,
or automatic font installation as part of this contract.

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
