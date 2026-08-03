# Presentation Themes and Output

Merman keeps four independent choices separate: host theme values, Merman presentation behavior, Mermaid configuration, and SVG output policy. A caller can combine them, but selecting one never silently selects another.

| Owner | Public input | Use it for |
| --- | --- | --- |
| Host theme | `Presentation::with_theme(...)` / `presentation.theme` | Semantic host colors, typography, and series colors |
| Merman presentation | `Presentation::with_profile(...)` / `presentation.profile` | Product-owned behavior such as the `merman-modern` Flowchart treatment |
| Mermaid configuration | `HeadlessRenderer::with_site_config(...)` / top-level `site_config` | Mermaid `theme`, `look`, layout, `themeVariables`, and family configuration |
| SVG output | `HeadlessRenderer::with_svg_pipeline(...)` / `svg` | Parity, readable, or `resvg-safe` post-processing and output-specific policy |

The default renderer remains Mermaid-parity oriented. An empty presentation is a no-op.

## Rust API

```rust
use merman::svg::{
    HeadlessRenderer, HostTheme, HostThemePreset, Presentation, PresentationProfile, SvgPipeline,
};

let presentation = Presentation::new()
    .with_profile(PresentationProfile::MermanModern)
    .with_theme(HostTheme::from_preset(HostThemePreset::OneDark));

let renderer = HeadlessRenderer::new()
    .with_presentation(presentation)
    .with_svg_pipeline(SvgPipeline::resvg_safe())
    .with_diagram_id("preview");
let svg = renderer.render_svg_sync(source)?;
```

`HostTheme` can also be built from semantic roles rather than a bundled preset. Role IDs describe host intent such as `canvas`, `surface-alt`, `text`, `line`, `edge-label-background`, `actor-text`, `error`, and `success`; the compiler maps them to Mermaid configuration owned by the relevant diagram families.

```rust
use merman::svg::{HostTheme, HostThemeAppearance, ThemeRole};

let theme = HostTheme::new()
    .with_appearance(HostThemeAppearance::Dark)
    .try_with_font_family("Inter, system-ui, sans-serif")?
    .try_with_role(ThemeRole::Canvas, "#0f172a")?
    .try_with_role(ThemeRole::Text, "#e5e7eb")?
    .try_with_role(ThemeRole::Line, "#94a3b8")?;
```

Bundled theme IDs are `editor-light`, `editor-dark`, `one-dark`, `gruvbox-light`, `gruvbox-dark`, `ayu-light`, and `ayu-dark`. The One Dark, Gruvbox, and Ayu presets are Merman's semantic mappings inspired by those color systems; they are not claims of byte-for-byte identity with a particular editor distribution.

Bundled presets keep normal and subtle text at a minimum 4.5:1 contrast against their canvas and structural line colors at a minimum 3:1. Palette labels independently choose black or white by the higher WCAG contrast ratio. Sequence actor and label-box variables follow the actor-specific roles, while Gantt done and critical tasks keep a readable neutral fill and express state through their semantic border color.

`merman-modern` is a presentation profile, not a theme preset. It selects Redux/slate Mermaid defaults, Neo look, an ELK default for ordinary Flowcharts, and Merman-owned Flowchart SVG behavior. A build without `layout-elk` can still discover and select the profile for diagrams that do not require that aspect. Use the SVG plan or presentation catalog to detect blocked aspects for the actual artifact and diagram.

## Options JSON

Bindings use Options JSON schema 2:

```json
{
  "version": 2,
  "presentation": {
    "profile": "merman-modern",
    "theme": {
      "preset": "one-dark",
      "font_family": "Inter, system-ui, sans-serif",
      "roles": {
        "canvas": "#0f172a",
        "text": "#e5e7eb",
        "line": "#94a3b8"
      },
      "series_palette": ["#60a5fa", "#34d399", "#f59e0b"]
    }
  },
  "site_config": {
    "flowchart": {
      "defaultRenderer": "dagre-wrapper"
    }
  },
  "svg": {
    "pipeline": "resvg-safe"
  }
}
```

Raw Mermaid overrides belong only at top-level `site_config`. Output choices belong only under `svg`. The removed `host_theme` group is rejected with a migration error instead of being accepted as a compatibility alias.

## Precedence

Merman materializes configuration in this order:

1. The renderer's base engine configuration.
2. Defaults selected by `presentation.profile`.
3. Explicit values from `presentation.theme`.
4. Explicit top-level `site_config` layers.
5. Diagram frontmatter and directives.
6. The independently selected SVG output pipeline after rendering.

Because the owners are stored separately, `with_site_config(...).with_presentation(...)` and the reverse builder call order produce the same effective configuration. Explicit Mermaid configuration and source-local configuration win over presentation defaults.

## Discovery

Rust callers enumerate built-in values through `theme_preset_descriptors()` and `presentation_profile_descriptors()`. Native and Web callers use the `presentation-catalog` metadata payload. Catalog IDs are open strings: consumers must tolerate future presets, profiles, aspect kinds, and capability requirements instead of treating the current list as a closed enum.

The catalog reports artifact-level availability. `svg-plan-json` reports the operation-specific result after the diagram family, effective renderer, explicit overrides, and compiled capabilities are known.
