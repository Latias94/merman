---
name: design-merman-theme
description: Create, refine, or audit a Merman Theme v2 preset or custom document. Use for semantic color, paired appearances, categorical palettes, typography, materials, and rendered previews.
---

# Design Merman Themes

1. Identify where the diagram will be read, the families it serves, the target outputs, and the intended hierarchy. Choose light or dark, or design the two appearances together. For material themes, choose a single recognizable texture or depth treatment. Sketch the palette with enough distinct data inks and fills for the example.
2. Author an editable Theme v2 JSON document. Start with `merman-cli theme export <id> --minimal` when a preset already has the desired structure. For a complete preset use `merman-cli theme export <id>`. Use [the schema](../../../schemas/theme-v2.schema.json) to look up fields and [the authoring guide](../../../docs/rendering/custom-diagram-themes.md) for application examples.
3. Start with seed colors and set exact roles only where the seed's derived paint misses the brief. Use appearance layers for light and dark paint changes. Use family overrides for design decisions a diagram family needs to make differently, such as more chart separation. Keep overrides small and keep the original document editable. Confirm the selected ID and appearance with `merman-cli theme list`.
4. Run `merman-cli theme check theme.json --json`, then render the exact saved document with `merman-cli render diagram.mmd --theme theme.json --appearance <light|dark> -o preview.svg`. Check each declared appearance. For a cross-family design, render the user's diagram and representative graph, sequence, class, and chart scenes. Use the installed fonts and output format of the intended host when checking measurements.
5. Inspect the actual previews for text contrast, labels over containers, canvas coverage, clipping, series separation, and depth. Recheck those details in the requested raster, PDF, or terminal target when applicable. Iterate on the saved document until those previews agree with the brief. Deliver the JSON, representative previews, and the appearance, fonts, and formats checked. Store temporary comparisons in `target/`.

For a compact starting point, use [starter.theme.json](assets/starter.theme.json) with [Flowchart](assets/flowchart.mmd), [Sequence](assets/sequence.mmd), and [XY Chart](assets/xychart.mmd).
