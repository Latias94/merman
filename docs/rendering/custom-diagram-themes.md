# Theme v2

Merman native themes share a canvas, semantic paints, categorical colors, typography, and form across diagram families. Start from a preset to edit a few settings or export the full design. [Schema](../../schemas/theme-v2.schema.json)

## Select and customize a theme

```sh
merman-cli theme list
merman-cli render diagram.mmd --theme studio --appearance dark -o diagram.svg
merman-cli theme export studio -o studio.theme.json
merman-cli theme check studio.theme.json --json
merman-cli render diagram.mmd --theme studio.theme.json -o diagram.svg
```

A document can extend a preset and override only the values it needs. This example changes the accent and shape radius for both appearances.

```json
{
  "schema_version": 2,
  "extends": "studio",
  "seed": { "accent": "#0052CC" },
  "form": { "radius": 8 }
}
```

Themes may declare `light`, `dark`, or both. Set `default_appearance` to choose the default of a full document. `theme check` reports syntax, schema, and contrast issues. Add `--strict-contrast` to fail on a contrast warning.

In the Playground, open **Theme library** to browse the installed presets. Select a design and preview its appearances on a Flowchart, Sequence, Pie, or your current diagram. **Apply theme** uses that choice in the workspace. A `?theme-design=<preset-id>` link opens the same preview and can accompany a shared workspace URL fragment. [Preview Riso](https://frankorz.com/merman/?theme-design=riso)

For a classic Mermaid look, pass its name to CLI `--theme` or set `site_config.theme` in bindings. When you choose a native theme, its settings provide global visual defaults. To give an explicitly authored Mermaid theme priority, pass `--theme-precedence source`. Local styles such as Flowchart `classDef`, `style`, and `linkStyle` keep their precedence over native defaults.

## Graphics and terminal output

Use `--background '#112233'` or `--background transparent` to replace the base of the diagram canvas. Theme textures remain visible. Raster and PDF matte settings belong to the destination output.

Set `canvas.padding_px` in an appearance to add space around the diagram inside the SVG viewport. The canvas paint and textures fill the expanded area.

The terminal palette uses the same native theme when you render ASCII or Unicode with `--ascii-color truecolor` or `--ascii-color ansi256`. A binding option `ascii.theme` can supply a separate terminal palette.

## Use a theme in an application

```rust
use merman::theme::{Appearance, BuiltinPreset, ThemeSelection};
use merman::{OperationControl, RenderRequest, Renderer, SvgRequest};

let theme = ThemeSelection::preset(BuiltinPreset::Studio, Some(Appearance::Dark));
let request = SvgRequest::default().with_native_theme(theme);
let svg = Renderer::new().render_svg_data(RenderRequest::svg(
    "flowchart LR\nA --> B", OperationControl::new(), request,
))?;
# Ok::<(), merman::RenderError>(())
```

To reuse an editable document across requests, call `ThemeCompiler::compile_json` and pass the resulting shared theme to `ThemeSelection::compiled`. Each graphical request, including one made from prepared source, can select its own theme.

Use `themeCatalog()` to populate a theme picker in Web and Node. `exportThemePreset(id)` returns an editable document. `resolveTheme(document, options)` accepts an object or raw JSON and reports compilation notices or a document-relative diagnostic. The Playground theme workshop offers preset selection, color and form controls, JSON editing, contrast diagnostics, and preview.

[Binding options](../bindings/OPTIONS_JSON.md#diagram-theme) · [Family support](diagram-theme-coverage.md) · [Rust example](../../crates/merman/examples/custom_diagram_theme.rs)
