# Host Theme Profiles

`HostThemeProfile` is an opt-in theme adapter for editor and application hosts. It lets a host map UI theme roles into Mermaid-compatible `themeVariables`, diagram config defaults, and an SVG output pipeline without changing the default Mermaid-parity renderer.

Default `render_svg_sync` stays parity-oriented. Use a profile only when a host wants product-owned preview or raster output.

## Rust API

```rust
use merman::render::{
    HeadlessRenderer, HostThemeOutput, HostThemeProfile, HostThemeRoles,
};

let profile = HostThemeProfile::builder()
    .font_family("Inter, system-ui, sans-serif")
    .roles(HostThemeRoles {
        canvas: Some("#0f172a".to_string()),
        surface: Some("#111827".to_string()),
        text: Some("#e5e7eb".to_string()),
        border: Some("#475569".to_string()),
        line: Some("#94a3b8".to_string()),
        success: Some("#34d399".to_string()),
        ..HostThemeRoles::default()
    })
    .series_palette(["#60a5fa", "#34d399", "#f59e0b"])
    .output(HostThemeOutput::resvg_safe_editor())
    .build();

let compiled = profile.compile();
let renderer = HeadlessRenderer::new()
    .with_compiled_host_theme(&compiled)
    .with_diagram_id("preview");
let svg = renderer.render_svg_with_pipeline_sync(source, &compiled.pipeline())?;
```

## Precedence

The profile compiles into normal Mermaid and SVG output settings:

1. Mermaid defaults from the pinned 11.15.0 baseline.
2. `HostThemeProfile` derived config.
3. Explicit profile `theme_variables` and `site_config` overrides.
4. Explicit caller `site_config` or diagram init/frontmatter config.
5. Host SVG output postprocessors such as root background and scoped CSS.
6. Explicit binding `svg.*` options when using `options_json`.

Use raw Mermaid `site_config`, `themeVariables`, `themeCSS`, or `svg.scoped_css` when a host needs selector-level control. The profile is a convenience and consistency layer, not a replacement for Mermaid configuration.

## Binding JSON

Bindings accept the same profile through `host_theme`:

```json
{
  "host_theme": {
    "appearance": "dark",
    "font_family": "Inter, system-ui, sans-serif",
    "roles": {
      "canvas": "#0f172a",
      "surface": "#111827",
      "text": "#e5e7eb",
      "border": "#475569",
      "line": "#94a3b8",
      "success": "#34d399"
    },
    "series_palette": ["#60a5fa", "#34d399", "#f59e0b"],
    "output": {
      "pipeline": "resvg-safe",
      "root_background": "canvas",
      "drop_native_duplicate_fallbacks": true,
      "css_override_policy": "strip-existing-important"
    }
  }
}
```

`appearance` accepts `light` or `dark`. `output.pipeline` accepts `parity`, `readable`, or `resvg-safe`. `output.root_background` accepts `none`, `canvas`, or a CSS color value. An empty `{ "host_theme": {} }` is a no-op and does not force Mermaid `theme=base`.

## Design Notes

The profile intentionally avoids editor-specific concepts such as player colors, GPUI color types, or product-owned class names. Hosts can still append custom `SvgPostprocessor` passes for those semantics.

`resvg-safe` output includes a best-effort fallback for HTML labels. When labels do not carry inline color, the fallback text inherits nearby Mermaid CSS or the root SVG fill before using its legacy default, so dark editor previews remain readable.

Series palette is mapped to Mermaid's existing palette entry points such as `cScale*`, `git*`, `pie*`, `venn*`, `fillType*`, and `xyChart.plotColorPalette`. Diagram families with data-keyed palettes, such as Sankey node ids, still need raw diagram config or a host postprocessor.
