# Merman theme authoring

The editable envelope is `{"schema_version":1,"kind":"design","design":{...}}`.
`complete_spec` is a concrete snapshot variant. Preserve it on import unless the user requests a
conversion; equal literal values do not establish shared authoring intent.

## Authoritative sources

In a Merman checkout, consult the [theme guide](../../../../docs/rendering/custom-diagram-themes.md)
for current behavior and [design.rs](../../../../crates/merman-theme-contract/src/design.rs) for
exact fields. [ADR 0095](../../../../docs/adr/0095-general-theme-authoring-and-retained-bindings.md)
explains document ownership. The loaded engine's catalog and same-render evidence determine its
actual capabilities; a schema field is not proof of an active consumer. Consult the guide's
capability limitations before designing around an uncertain mechanism.

## Compose the design

| Authored section | Purpose |
| --- | --- |
| `tokens` | Optional typed named values with `label`, `group`, and `description` |
| `styles` | Defaults keyed by semantic target, such as `node`, `node-label`, or `edge` |
| `rules` | Ordered exceptions with explicit family, target, variant, or ordinal selectors |
| `palettes` | Target-specific series colors |
| `typography` | Default and family-specific text styles |
| `canvas` | Exported base paint, ordered decorative layers, and bleed |
| `mermaid` | Restricted theme compatibility values, subject to source/config precedence |
| `appearances`, `default_appearance` | Optional concrete light/dark layers and the required declared default |

References are exact local values of the form `{"ref":"id"}`. They are typed, can occur inside
larger paints/effects, and cannot be expressions or arbitrary paths. A `color` token fits a gradient
stop; a `paint` token fits an entire fill. Use Rust diagnostics to resolve consumer-domain errors.

Separate body and label paint, and body and label effects. A dark surface with a light label needs
both targets addressed. Shared roles should have intentional consumers; naming a value `text`
does not automatically connect every family label to it.

Use token descriptions to explain useful relationships: a registration color may connect a body
offset and an activation, while equal plate and connector colors can remain separate tokens for
independent editing. A small graph of these relationships is more useful than a token for every
literal. Keep geometry and data meaning in their existing source/layout owners.

## Ownership and overrides

Request `theme` is a whole-value selection: omission inherits the engine's compiled theme,
`null` clears it, and a supplied recipe/selection replaces it completely. It is excluded from
the generic options deep merge; a partial recipe is not a request patch. See the
[binding contract](../../../../docs/bindings/OPTIONS_JSON.md#diagram-theme).

Within a recipe, editing a token changes only its explicitly bound consumers. To detach one
consumer, replace its reference with a local copy of the authored value, retaining nested
references that should still follow other tokens. Matching literals do not create bindings.

Defaults lower first, then ordered rules, then palettes. Later matching facets can override
earlier ones; a family selector adds scope, not CSS specificity. Omitted patch fields leave the
property unspecified. On clearable facets, `null` means Clear: restore family baseline behavior,
rather than reveal the preceding matching rule. Check the actual terminal's Clear support.
Supplied lists replace rather than implicitly append. Preserve rule order during unrelated
edits, and compare authored JSON rather than recoloring a resolved snapshot.

Source styles and explicit Mermaid variables retain property-specific ownership. An authored
source theme owns its materialized paint palette for present roles; a site/base theme supplies
fallback paint. Typography, geometry, effects, and canvas keep their own owners. Preserve source
init/frontmatter and site configuration separately, along with look and layout; moving values
between owners can change semantics. The [guide's ownership table](../../../../docs/rendering/custom-diagram-themes.md#family-configuration-and-compiled-rules)
owns these distinctions.

## Paired appearances

Use one `ThemeRecipeV1` with its optional `design.appearances` map and
`design.default_appearance`. Single-appearance artwork may declare one entry; an ordinary
concrete design without these fields remains unclassified.

```json
{
  "schema_version": 1,
  "kind": "design",
  "design": {
    "styles": {
      "node": { "fill": { "ref": "surface" } },
      "node-label": { "fill": { "ref": "ink" } }
    },
    "canvas": { "base": { "ref": "surface" } },
    "appearances": {
      "light": {
        "tokens": {
          "surface": { "type": "paint", "value": "#fafbfc" },
          "ink": { "type": "paint", "value": "#20242c" }
        }
      },
      "dark": {
        "tokens": {
          "surface": { "type": "paint", "value": "#20242c" },
          "ink": { "type": "paint", "value": "#fafbfc" }
        }
      }
    },
    "default_appearance": "light"
  }
}
```

This seed covers Flowchart node roles, not every family. For a reviewed multi-family example,
inspect [Reading](../../../../playground/src/themes/recipes/reading.theme.json).
Local tokens replace matching IDs; local styles follow shared defaults and rules, then local
rules run in order. Palettes replace an entire target entry. Canvas and typography replace
their entire section; Mermaid compatibility merges only declared fields and variable entries.
Assets and requirements stay shared. No recursive JSON merge or automatic color inversion occurs.

All declared appearances must pass admission, including the inactive branch. Inspect the full
recipe, then materialize/render each concrete appearance; a broken dark branch cannot become a
valid light-only import. Keep the complete recipe for exchange. A `complete_spec` snapshot loses
authoring relationships and appearance selection.

Catalog groups expose supported/default appearances and per-appearance metadata, including
`concrete_preset_id`. Select canonical `editor`, `gruvbox`, or `ayu` with an appearance; concrete
IDs remain pinned. Mermaid Default and Dark stay independent. Only Neo, Redux, and Redux Color
have declared upstream pairs. `design.mermaid.dark_mode` remains a compatibility value.
Keep selection and export conditions explicit as described in
[runtime.md](runtime.md#appearance-and-viewer-backing).

## Backgrounds and shadows

Canvas paint belongs to the exported image. Preview backing belongs to the viewer and is not part
of the recipe. Omission retains base renderer behavior; `"transparent"` explicitly requests no
base paint but retains decorative layers. Clearing all background paint requires removing the
layers as an intentional change. Node surfaces and diagram-owned background shapes are separate.
The canvas base is paint, not a patch field: use `"transparent"`, not `null`.

A `shadow-stack` uses `composition: "independent"` for shadows from the original shape, or
`"cascade"` for successive effects on the preceding result. `offset_x`, `offset_y`, and
`std_deviation` are pixel lengths; zero deviation makes a hard shadow. Deviation is Gaussian sigma,
not CSS blur radius. There is no spread, inset, or backdrop-shadow convenience field.

Flowchart Classic and Neo both consume supported body, edge and label effects with ELK or Dagre.
Body shadows cover rectangles, rounded rectangles, diamonds, circles and double circles; hand-drawn
geometry and other shapes can leave effects unapplied. Preserve the requested look and source
styles, and inspect execution diagnostics for the actual input. `resvg-safe` is an SVG pipeline,
not proof that all theme facets or PDF conversion are portable. Validate the requested target
separately.

On supported Flowchart bodies, an explicit effect replaces the look's default shadow; an omitted
effect retains it. An explicit effect Clear also suppresses the look shadow. Source filter
ownership still takes precedence. Body and label effects remain independent, including Clear.

`design.mermaid` admits `theme`, `dark_mode`, and a scalar `variables` map; it is not arbitrary
Mermaid configuration. Put `flowchart.look` and other diagram configuration in the accompanying
render options or source frontmatter. When the design needs those settings, deliver and explain
them alongside the JSON; importing the recipe preserves the diagram's look.

## Check the result

The materialization result includes resolved compiler input and declared reference uses. Uses
describe relationships, not painted objects or winning rules. A successful render's
`metadata.theme_execution_evidence` provides theme and target states plus diagnostics for that
execution. Missing/unknown evidence and empty diagnostics do not certify portability.

For an uncertain mechanism, make a focused support query, for example:

```json
{
  "schema_version": 1,
  "family": "flowchart",
  "output": "standalone-svg",
  "subject": { "kind": "rule", "target": "node", "facet": "effect" }
}
```

Use the rendered labels and sources to assess readability. Include long labels, branches, notes,
and multiple series when they matter to the brief. Font names require fonts on the receiving
host; naming a font in JSON does not embed it. Deliver font requirements and asset licensing when
used. Follow [review.md](review.md) to distinguish requested facets, actual paint, visual bounds,
and output-target admission, including the `canvas.bleed` no-consumer example.
