# ADR 0102: Native Theme v2

- Accepted for the Theme v2 development branch.
- Supersedes [ADR 0095](0095-general-theme-authoring-and-retained-bindings.md) for native theme authoring.
- [ADR 0104](0104-paint-scene-ir-and-dual-output-profiles.md) defines how resolved paint is used by the opt-in compact SVG profile.

## Decision

A schema-2 JSON document defines semantic paints, categorical colors, typography, form, canvas texture, and elevation. The compiler derives missing roles in OKLCH and reports contrast issues before rendering. Presets are editable documents from the same registry. A document can extend one built-in preset and declare light and dark appearances. The host selects the appearance or uses the document's default.

Mermaid configuration follows the existing source and site configuration path. A native theme enters at layout and rendering, where each family maps shared roles onto its own objects. The family exposes supported settings as a static bit mask. A layout setting or decoration can report a capability when the family consumes it. Shoal adapters pass resolved spacing, contours, and marker dimensions through the existing measured graph and route interfaces where the family has those inputs before layout. [Family coverage](../rendering/diagram-theme-coverage.md)

Prepared source owns parse-time facts and can be rendered under different visual choices. Host theme selection owns global defaults, with an explicit source-first mode for diagrams that declare their own Mermaid theme. Authored local styles retain their precedence on the elements they address. The canvas override changes the diagram's base paint. Export matte belongs to the destination raster or page.

The render result exposes the selected mode and compiler notices. Graphical export continues to enforce its resource limits and validate the SVG and fonts it actually consumes.

[Theme guide](../rendering/custom-diagram-themes.md) · [Theme v2 schema](../../schemas/theme-v2.schema.json)
