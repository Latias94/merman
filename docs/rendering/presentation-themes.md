# Presentation Themes and Output

Merman keeps four rendering choices separate: host semantic theme data, Merman presentation
behavior, Mermaid configuration, and SVG output policy. `HeadlessRenderer` owns these inputs
directly; there is no public aggregate object whose only job is to forward optional fields.

| Owner | Public input | Use it for |
| --- | --- | --- |
| Host theme | `HeadlessRenderer::with_host_theme(...)` / `presentation.theme` | Portable semantic colors, typography, and series colors |
| Merman presentation | `HeadlessRenderer::with_presentation_profile(...)` / `presentation.profile` | Product-owned behavior such as the `merman-modern` Flowchart treatment |
| Mermaid configuration | `HeadlessRenderer::with_site_config(...)` / top-level `site_config` | Mermaid `theme`, `look`, layout, `themeVariables`, `themeCSS`, and family configuration |
| SVG output | `HeadlessRenderer::with_svg_pipeline(...)` / `svg` | Parity, readable, or `resvg-safe` post-processing and output-specific policy |

The default renderer remains Mermaid-parity oriented. Omitting both profile and host theme is a
no-op.

## Rust API

```rust
use merman::svg::{
    HeadlessRenderer, HostTheme, HostThemePreset, PresentationProfile, SvgPipeline,
};

let renderer = HeadlessRenderer::new()
    .with_presentation_profile(PresentationProfile::MermanModern)
    .with_host_theme(HostTheme::from_preset(HostThemePreset::OneDark))
    .with_svg_pipeline(SvgPipeline::resvg_safe())
    .with_diagram_id("preview");
let svg = renderer.render_svg_sync(source)?;
```

`HostTheme` can also be built from semantic roles rather than a bundled preset. Role IDs describe
host intent such as `canvas`, `surface-alt`, `text`, `line`, `edge-label-background`, `actor-text`,
`error`, and `success`; the compiler maps them to Mermaid configuration owned by the relevant
diagram families.

```rust
use merman::svg::{HostTheme, HostThemeAppearance, ThemeRole};

let theme = HostTheme::new()
    .with_appearance(HostThemeAppearance::Dark)
    .try_with_font_family("Inter, system-ui, sans-serif")?
    .try_with_role(ThemeRole::Canvas, "#0f172a")?
    .try_with_role(ThemeRole::Text, "#e5e7eb")?
    .try_with_role(ThemeRole::Line, "#94a3b8")?;
```

See [`presentation_profile.rs`](../../crates/merman/examples/presentation_profile.rs) for a
ready-made profile and preset combination, or
[`custom_presentation_theme.rs`](../../crates/merman/examples/custom_presentation_theme.rs) for a
self-contained semantic-role mapping.

Bundled theme IDs are `editor-light`, `editor-dark`, `one-dark`, `gruvbox-light`, `gruvbox-dark`,
`ayu-light`, and `ayu-dark`. The One Dark, Gruvbox, and Ayu presets are Merman's semantic mappings
inspired by those color systems; they are not claims of byte-for-byte identity with a particular
editor distribution.

Bundled presets keep normal and subtle text at a minimum 4.5:1 contrast against their canvas and
structural line colors at a minimum 3:1. Palette labels independently choose black or white by the
higher WCAG contrast ratio. Sequence actor and label-box variables follow the actor-specific roles,
while Gantt done and critical tasks keep a readable neutral fill and express state through their
semantic border color.

`merman-modern` is a presentation profile, not a theme preset. It owns Neo behavior, an ELK default
for ordinary Flowcharts, and Merman-owned Flowchart SVG behavior. For compatibility it also supplies
a Redux/slate visual fallback only when neither a non-empty `HostTheme` nor a later site-level
Mermaid `theme` owns the appearance. Selecting a host theme or setting `site_config.theme` replaces
that fallback without disabling the profile behavior. A partial `themeVariables` overlay without an
explicit `theme` intentionally customizes the fallback instead of replacing it.

A build without `layout-elk` can still discover and select the profile for diagrams that do not
require that aspect. Use the SVG plan or presentation catalog to detect blocked aspects for the
actual artifact and diagram.

## Complete Visual Recipes

`HostTheme` is intentionally a semantic design-system adapter, not an arbitrary Mermaid theme or a
CSS/SVG document builder. This keeps host tokens portable across diagram families and avoids mixing
layout, CSS, export, resources, and product behavior into a preset lattice.

Themes such as those in `modern_mermaid` are composite recipes. Their pieces map to Merman as
follows:

| Recipe component | Merman owner | Built-in support |
| --- | --- | --- |
| Semantic palette and typography intent | `HostTheme` | Yes |
| Exact Mermaid variables and per-family settings | `MermaidConfig` / `site_config` | Yes |
| Selector-heavy theme CSS | `site_config.themeCSS` | Yes; selectors rooted at `svg[...]` are scoped to the actual root SVG |
| Solid exported background | `SvgOutputPolicy::root_background_color` | Yes |
| Gradient or patterned canvas | Host UI or a custom `SvgPostprocessor` | Not represented by `HostTheme` |
| SVG filters, `<defs>`, and structural mutations | Custom `SvgPostprocessor` before terminal validation | Rust extension point only; not declarative Options JSON |
| External font files and exact glyph metrics | Host resource loading plus a matching text measurer | A font-family name alone is insufficient |
| Annotation overlay colors and controls | Host application | Outside the Mermaid SVG theme contract |

Consequently, token-oriented themes such as Linear Light can be represented mostly through
`HostTheme` plus a solid output background. CSS-heavy themes such as Brutalist should use exact
`themeVariables` and `themeCSS` through `site_config`; forcing them through semantic roles would lose
selector-specific borders, radii, shadows, and family exceptions. Structural themes such as Hand
Drawn additionally need a custom SVG pass for filter definitions. The Rust pipeline can express
that pass, while the cross-language Options JSON contract deliberately cannot accept arbitrary SVG
markup or executable postprocessors.

CSS is emitted after layout. If a theme changes font family or font size, provide those values as
Mermaid configuration or `HostTheme` inputs and select a text measurer with matching metrics. A CSS
font override alone can change paint output without changing wrapping or bounding boxes.

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

Raw Mermaid overrides belong only at top-level `site_config`. Output choices belong only under
`svg`. The removed `host_theme` group is rejected with a migration error instead of being accepted
as a compatibility alias. The wire keys `presentation.profile` and `presentation.theme` remain
stable even though the Rust facade no longer exposes a `Presentation` container.

## Precedence

Merman materializes configuration in this order:

1. The renderer's base engine configuration.
2. Behavior defaults selected by `presentation.profile`.
3. The profile's visual fallback, only when no later host or site-level theme owner exists.
4. Explicit values from `presentation.theme` / `with_host_theme(...)`.
5. Explicit top-level `site_config` layers.
6. Diagram frontmatter and directives, subject to Merman's hardened secure-key policy.
7. The independently selected SVG output pipeline after rendering.

Because the owners are stored separately, builder call order does not change this precedence.
`with_site_config(...).with_presentation_profile(...)` and the reverse order produce the same
effective configuration. A non-empty site-level `theme` replaces the profile visual fallback;
explicit site configuration then overlays the selected theme normally.

Diagram-local configuration can override non-secure Mermaid fields. It cannot override hardened
site-owned `fontFamily`, `themeCSS`, or `themeVariables`, so applications that expose complete theme
selection should apply it at the renderer/site-config layer rather than inside diagram source.
