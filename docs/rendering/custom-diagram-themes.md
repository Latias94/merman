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

The current palette presets advertise no dedicated Modern Mermaid reproduction. A successful SVG
render is not proof of readable dense labels, complete effects, or PNG/PDF portability.
`describeThemeSupport()` answers mechanism questions for a family and target; it does not inspect
and approve the visual design of a particular scene. Web `renderSvg()` currently returns only
the SVG string, and `svgPlanJson().ready` checks artifact capabilities rather than applied theme
facets. Do not use either as proof that every authored rule took effect. Exposing the actual
request's theme outcome through a convenient Web entry point remains a contract-freeze gate.
Web options do not yet expose the Rust facade's `RequirePortable` policy; `resvg-safe` selects a
pipeline and must not be presented as that strict policy.

There is no automatic cross-preset fallback. Outside a dedicated design scope, retain the selected
recipe and explain whether it has base-only or unreviewed styling. A host can offer an explicit
alternative. Unknown preset IDs and unsupported input schema versions are input errors, not reasons
to silently substitute a default. An unsupported effect must remain visible in support/admission
handling; choosing another preset must not conceal it.

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

This example changes explicit roles. It is **not** a global brand-color parameter. Current Cyberpunk
exports 68 rules, with independent node, actor, edge, marker, and data-series roles. Replacing every
identical color string can damage those distinctions. Convenient parameterization of a complete
complex preset remains under review before public contract freeze.

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
