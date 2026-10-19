# Merman theme authoring

The editable envelope is `{"schema_version":1,"kind":"design","design":{...}}`.
`complete_spec` is a concrete snapshot variant. Preserve it on import unless the user requests a
conversion; equal literal values do not establish shared authoring intent.

## Authoritative sources

In a Merman checkout, consult `docs/rendering/custom-diagram-themes.md` for runtime behavior and
`crates/merman-theme-contract/src/design.rs` for exact fields. `docs/adr/0095-general-theme-authoring-and-retained-bindings.md`
explains document ownership. The loaded engine's catalog and diagnostics determine its actual
capabilities; a field in the schema does not establish renderer support.

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

References are exact local values of the form `{"ref":"id"}`. They are typed, can occur inside
larger paints/effects, and cannot be expressions or arbitrary paths. A `color` token fits a gradient
stop; a `paint` token fits an entire fill. Use Rust diagnostics to resolve consumer-domain errors.

Separate body and label paint, and body and label effects. A dark surface with a light label needs
both targets addressed. Shared roles should have intentional consumers; naming a value `text`
does not automatically connect every family label to it.

Defaults lower first, then ordered rules, then palettes. Later matching facets can override
earlier ones; a family selector adds scope, not CSS specificity. Source styles and Mermaid
configuration retain their ownership and may supersede theme values. Preserve rule order during
unrelated edits, and compare the authored JSON rather than recoloring a resolved snapshot.

## Backgrounds and shadows

Canvas paint belongs to the exported image. Preview backing belongs to the viewer and is not part
of the recipe. Omission retains base renderer behavior; `"transparent"` explicitly requests no
base paint but retains decorative layers. Clearing all background paint requires removing the
layers as an intentional change. Node surfaces and diagram-owned background shapes are separate.

A `shadow-stack` uses `composition: "independent"` for shadows from the original shape, or
`"cascade"` for successive effects on the preceding result. `offset_x`, `offset_y`, and
`std_deviation` are pixel lengths; zero deviation makes a hard shadow. Deviation is Gaussian sigma,
not CSS blur radius. There is no spread, inset, or backdrop-shadow convenience field.

Flowchart Classic and Neo have different effect consumers. Current Flowchart body/edge/label
shadows require `flowchart.look: classic`; preserve the requested look, report this condition, and
change it only as an explicit design choice. `resvg-safe` is an SVG pipeline, not proof that all
theme facets or PDF conversion are portable. Validate the actual requested target separately.

`design.mermaid` admits `theme`, `dark_mode`, and a scalar `variables` map; it is not arbitrary
Mermaid configuration. Put `flowchart.look` and other diagram configuration in the accompanying
render options or source frontmatter. When the design needs those settings, deliver and explain
them alongside the JSON; importing the recipe alone does not select Classic.

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
host; portable JSON does not embed them. Deliver font requirements and asset licensing when used.
