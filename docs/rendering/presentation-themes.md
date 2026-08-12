# Diagram Themes and SVG Output

Alpha.4 exposes an **experimental** typed diagram-theme surface. A theme recipe is compiled from
`DiagramThemeSpec` (or one of the first-party `ThemePreset` values) into an immutable
`DiagramTheme`, then installed with `HeadlessRenderer::with_theme(...)`. This surface is not a
completed cross-target compatibility promise: compilation reports required capabilities, while
family, document, and export stages provide the evidence and target-specific admission decision.
The C6 representative SVG/PNG/PDF matrix is not yet proven.

Merman keeps independent concerns in separate owners:

| Owner | Public input | Use it for |
| --- | --- | --- |
| Compiled diagram theme | `DiagramThemeSpec` -> `DiagramThemeCompiler` -> `DiagramTheme`; `HeadlessRenderer::with_theme(...)` | Typed semantic styles, typography, canvas, effects, font assets, and the explicit Mermaid compatibility lane |
| Mermaid configuration | `HeadlessRenderer::with_site_config(...)` / bounded binding `site_config` | Official Mermaid `theme`, `themeVariables`, `look`, layout, and family configuration; raw `themeCSS` is limited to trusted Rust/native CLI hosts |
| Layout and runtime | `HeadlessRenderer::with_layout_options(...)`, `RenderEnvironment`, and explicit layout configuration | Container dimensions, text measurement, math, resource policy, and renderer selection |
| SVG output | `HeadlessRenderer::with_svg_pipeline(...)` / `svg` | Parity, readable, or `resvg-safe` post-processing and output policy |

There is no public `PresentationTheme`, `HostTheme`, `PresentationProfile`, or aggregate
`Presentation` replacement. Product recipes belong in the host application and compose these
owners explicitly.

## Rust API

Select a built-in typed preset and install the compiled value on a renderer:

```rust
use merman::svg::{DiagramThemeCompiler, HeadlessRenderer, SvgPipeline, ThemePreset};

let theme = DiagramThemeCompiler::new().compile_preset(ThemePreset::OneDark)?;
let renderer = HeadlessRenderer::new()
    .with_theme(theme)
    .with_svg_pipeline(SvgPipeline::resvg_safe())
    .with_vendored_text_measurer()
    .with_diagram_id("theme-preset-example");
let svg = renderer.render_svg_sync(source)?;
```

`ThemePreset` currently contains `editor-light`, `editor-dark`, `one-dark`, `gruvbox-light`,
`gruvbox-dark`, `ayu-light`, and `ayu-dark`. Presets are visual theme data. They do not select
`look: neo`, an ELK renderer, an SVG pipeline, or a product behavior profile. The preset's
Mermaid compatibility values are limited to the explicit compatibility lane owned by the theme.

For a design-system token record, use `ThemeTokens` as a small Rust-side adapter and convert it to
a complete spec before compiling:

```rust
use merman::svg::{DiagramThemeCompiler, HeadlessRenderer, ThemeTokens};

let spec = ThemeTokens::default()
    .with_canvas("#0f172a")?
    .with_surface("#111827")?
    .with_surface_alt("#1f2937")?
    .with_text("#e5e7eb")?
    .with_border("#475569")?
    .with_line("#94a3b8")?
    .with_series(["#60a5fa", "#34d399", "#f59e0b"])?.into_theme_spec();
let theme = DiagramThemeCompiler::new().compile(spec)?;
let renderer = HeadlessRenderer::new().with_theme(theme);
```

`DiagramThemeSpec` can also be assembled directly. Its typed sections are Mermaid compatibility,
typography, semantic rules and ordinal palettes, canvas, effects, resource assets, and declared
requirements. The compiler validates the complete recipe and returns a `DiagramTheme`; it does not
read CSS variables, selectors, or host application state.

## Independent Configuration

The useful parts of a product recipe that used to be bundled under a named profile are explicit:

```rust
use merman::svg::{DiagramThemeCompiler, HeadlessRenderer, ThemePreset};
use merman_core::MermaidConfig;
use serde_json::json;

let theme = DiagramThemeCompiler::new().compile_preset(ThemePreset::OneDark)?;
let renderer = HeadlessRenderer::new()
    .with_theme(theme)
    .with_site_config(MermaidConfig::from_value(json!({
        "look": "neo",
        "flowchart": { "defaultRenderer": "elk" }
    })));
```

The configuration above is illustrative: ELK must be available in the selected artifact, and
ordinary Flowcharts are the only diagrams that acquire that layout requirement. A theme never
silently selects either option. A custom SVG pipeline, root background, scoped CSS policy, or
postprocessor remains an explicit `SvgPipeline`/`SvgOutputPolicy` choice.

## Complete Visual Recipes

Typed theme input is intentionally bounded. Use the owner that matches the semantics of each
value:

| Recipe component | Merman owner | Status |
| --- | --- | --- |
| Semantic palette, typography, geometry, and ordinal series | `DiagramThemeSpec` / `ThemeTokens` | Experimental typed support |
| Layered canvas and bounded effects | `DiagramThemeSpec` | Experimental typed model; target evidence is still required |
| Exact Mermaid variables and per-family settings | `MermaidConfig` / `site_config` or `DiagramThemeSpec::mermaid` | Supported compatibility lanes, with Mermaid precedence |
| Selector-heavy `themeCSS` | Trusted Rust/native CLI `site_config.themeCSS` | Host capability lane; rejected by general bindings and not typed portability evidence |
| Solid exported background and SVG cleanup | `SvgOutputPolicy` / `SvgPipeline` | Explicit output policy |
| External fonts and exact glyph metrics | `ThemeAssets`/`FontCatalogSpec` plus a matching text-measurement policy | Resource- and target-dependent |
| Product annotations, editor overlays, and controls | Host application | Outside the diagram-theme contract |

Raw CSS, custom postprocessors, browser-only effects, and under-attested host text measurement can
produce document residuals (for example `Unverified` or `Failed`) and can make the final target
`HostDependent` or `Rejected`. `DocumentResidualReason` describes an unresolved mechanism; the
separate `TargetAdmissionStatus` describes the final output decision. They must not be described as
proof that a typed theme is portable. `RequirePortable` is an explicit host/output admission
request; it is not implied by successful compilation.

## Options JSON

Bindings use Options JSON schema 2. The alpha.4 theme field is top-level and is a closed tagged
union: exactly one of `preset` or `spec` must be present. An empty object, both members, or a null
member is invalid.

```json
{
  "version": 2,
  "theme": {
    "preset": "one-dark"
  },
  "site_config": {
    "look": "neo",
    "flowchart": { "defaultRenderer": "elk" }
  },
  "svg": {
    "pipeline": "resvg-safe"
  }
}
```

Use `theme.spec` for a complete typed recipe. The supported top-level sections are `mermaid`,
`typography`, `styles`, `canvas`, `effects`, `requirements`, and `assets`. Binding JSON rejects
unknown fields and unsupported enum values. `theme.spec.mermaid` accepts only the bounded Mermaid
compatibility values (`default`, `forest`, `dark`, `neutral`, `base`, `neo`, `neo-dark`, `redux`,
`redux-dark`, `redux-color`, or `redux-dark-color`) plus scalar string, number, and boolean
variables; renderer selection, `look`, layout, raw CSS, and output policy stay outside this section.

```json
{
  "theme": {
    "spec": {
      "mermaid": {
        "theme": "base",
        "dark_mode": true,
        "variables": { "primaryColor": "#2563eb" }
      },
      "typography": {
        "default": {
          "font_stack": ["Inter", "system-ui", "sans-serif"],
          "font_size_px": 14,
          "line_height": 1.4
        }
      },
      "styles": [
        {
          "kind": "rule",
          "target": "node",
          "family": "flowchart",
          "style": {
            "fill": "#111827",
            "stroke": { "paint": "#60a5fa", "width": 2 }
          }
        },
        {
          "kind": "ordinal-palette",
          "target": "chart-series",
          "colors": ["#60a5fa", "#34d399", "#f59e0b"]
        }
      ],
      "canvas": { "base": "#0f172a", "bleed": 8 },
      "effects": [],
      "requirements": { "capabilities": ["semantic-rules"] }
    }
  }
}
```

For a reusable binding engine, a constructor theme is compiled once. A request-level `theme`
object replaces that complete value; `null` clears it; omission inherits it. The theme object is
not deep-merged with the constructor theme. Resource ceilings and admission policy remain
constructor-owned and cannot be raised by a request.

The `theme-catalog` metadata entry describes the presets, semantic target IDs, capability IDs,
font source/container IDs, and theme resource limits available in the selected artifact. Query the
artifact catalog when a slim build may omit a capability.

## Precedence and CLI Compatibility

Configuration precedence is structural and independent of builder call order:

1. The base `Engine` configuration.
2. The compiled theme's explicit Mermaid compatibility values.
3. Explicit renderer or bounded binding `site_config`; general bindings cannot supply `themeCSS`
   or replace `secure`.
4. Diagram frontmatter and directives, subject to hardened secure keys.
5. Typed semantic rules, typography, canvas, effects, and resources are resolved by the family,
   document, and output stages that consume them; their evidence is reported separately from the
   Mermaid configuration object.
6. The selected SVG output pipeline runs after rendering and owns terminal output policy.

The official Mermaid CLI selector remains unchanged: `merman-cli mmdc -t/--theme` accepts the
upstream values `default`, `forest`, `dark`, and `neutral` (with an omitted value meaning
`default`). Merman's compiled selectors are separate: native render/batch commands use
`--theme-preset` or `--theme-file`, and binding callers use top-level `theme`.

The removed `presentation`, `host_theme`, `PresentationTheme`, `HostTheme`, and
`PresentationProfile::MermanModern` names are not compatibility aliases. Options JSON rejects the
removed groups with migration-oriented errors; choose top-level `theme`, `site_config`, explicit
layout configuration, and `svg` instead.
