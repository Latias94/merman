# Host Theme Profiles

`HostThemeProfile` is an opt-in theme adapter for editor and application hosts. It lets a host map UI theme roles into Mermaid-compatible `themeVariables`, diagram config defaults, and an SVG output pipeline without changing the default Mermaid-parity renderer.

Default `render_svg_sync` stays parity-oriented. Use a profile only when a host wants product-owned preview or raster output.

## Rust API

```rust
use merman::render::{HeadlessRenderer, HostThemePreset, HostThemeProfile};

let profile = HostThemeProfile::from_preset(HostThemePreset::OneDark);
let renderer = HeadlessRenderer::new()
    .with_host_theme(&profile)
    .with_diagram_id("preview");
let svg = renderer.render_svg_sync(source)?;
```

Use request-scoped helpers when only one diagram render should use a host theme:

```rust
use merman::render::{HeadlessRenderer, HostThemePreset, HostThemeProfile};

let renderer = HeadlessRenderer::new().with_diagram_id("preview");
let profile = HostThemeProfile::from_preset(HostThemePreset::GruvboxDark);
let svg = renderer.render_svg_with_host_theme_sync(source, &profile)?;
```

`render_svg_with_site_config_sync(...)`, `render_svg_with_host_theme_sync(...)`, and
`render_svg_with_compiled_host_theme_sync(...)` do not mutate the renderer. They apply extra
Mermaid defaults and host output settings only for the current render call; diagram frontmatter
and `%%{init}%%` directives still merge on top of those defaults.

`HostThemePreset::ALL` exposes the built-in host presets for Rust UIs, `supported_host_theme_presets()`
returns their stable string names, and
`HostThemePreset::as_str()` returns the canonical `host_theme.preset` string used by bindings.
These names are not Mermaid core theme names.

`HeadlessRenderer::with_host_theme(...)` and `with_compiled_host_theme(...)` both apply the
compiled Mermaid config and install the profile's SVG output pipeline as the renderer default.
Use `render_svg_with_pipeline_sync(...)` only when a call needs to override that pipeline.

## Precedence

The profile compiles into normal Mermaid and SVG output settings:

1. Mermaid defaults from the pinned 11.15.0 baseline.
2. `HostThemeProfile` derived config.
3. Explicit profile `theme_variables` and `site_config` overrides.
4. Explicit caller `site_config` or diagram init/frontmatter config.
5. Host SVG output postprocessors such as root background and scoped CSS.
6. Explicit Rust `render_svg_with_pipeline_sync(...)` calls or binding `svg.*` options.

Use raw Mermaid `site_config`, `themeVariables`, `themeCSS`, or `svg.scoped_css` when a host needs selector-level control. The profile is a convenience and consistency layer, not a replacement for Mermaid configuration.

Use `HostThemeProfile::builder()` when a host wants to provide every role manually instead of starting from a preset.

## Binding JSON

Bindings accept the same profile through `host_theme`:

```json
{
  "host_theme": {
    "preset": "one-dark",
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

`preset` accepts `editor-light`, `editor-dark`, `one-dark`, `gruvbox-light`, `gruvbox-dark`, `ayu-light`, or `ayu-dark`. Explicit `roles`, `series_palette`, `themeVariables`, `site_config`, and `output` values override the preset. These host presets are separate from Mermaid core theme names returned by `supported_themes()`. Built-in host presets default to editor-safe `resvg-safe` output; an empty `{ "host_theme": {} }` remains a no-op.

## Design Notes

The profile intentionally avoids editor-specific concepts such as player colors, GPUI color types, or product-owned class names. Hosts can still append custom `SvgPostprocessor` passes for those semantics.

`resvg-safe` output includes a best-effort fallback for HTML labels. When labels do not carry inline color, the fallback text inherits nearby Mermaid CSS or the root SVG fill before using its legacy default, so dark editor previews remain readable.

Series palette is mapped to Mermaid's existing palette entry points such as `cScale*`, `git*`, `pie*`, `venn*`, `fillType*`, and `xyChart.plotColorPalette`. Diagram families with data-keyed palettes, such as Sankey node ids, still need raw diagram config or a host postprocessor.

For a stronger visual showcase, run:

```bash
cargo run -p merman --features render --example example_13_stylized_theme_showcase > showcase.svg
```
