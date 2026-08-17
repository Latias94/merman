# Diagram Themes and SVG Output

The unreleased branch after alpha.5 exposes an **experimental** typed diagram-theme surface. A
theme recipe is compiled from
`DiagramThemeSpec` (or one of the first-party `ThemePreset` values) into an immutable
`DiagramTheme`, then attached to the operation `RenderRequest`. This surface is not a
completed cross-target compatibility promise: compilation reports required capabilities, while
family, document, and export stages provide the evidence and target-specific admission decision.
The private C6a representative matrix covers Flowchart, State, and Sequence across Standalone SVG
and PNG. It does not imply equivalent support for every diagram family or output target.

Merman keeps independent concerns in separate owners:

| Owner | Public input | Use it for |
| --- | --- | --- |
| Versioned authoring | `ThemeDefinitionV1` -> `ThemeMaterializer` -> `DiagramThemeSpecWireV1` | Compact cross-family tokens plus ordered authored rules and deterministic materialization identity |
| Compiled diagram theme | `DiagramThemeSpecWireV1` / `DiagramThemeSpec` -> `DiagramThemeCompiler` -> `DiagramTheme`; `RenderRequest::with_theme(...)` | Typed semantic styles, typography, canvas, effects, font assets, and the explicit Mermaid compatibility lane |
| Mermaid configuration | `Engine::with_site_config(...)` / top-level `site_config` | Mermaid `theme`, `look`, layout, `themeVariables`, and family configuration |
| Layout and runtime | `Renderer`, `RenderRequest`, `SvgRequest`, and explicit operation control | Container dimensions, text measurement, math, resource policy, renderer selection, cancellation, and deadlines |
| SVG output | `SvgRequest.pipeline` / `svg` | Parity, readable, or `resvg-safe` post-processing and output-specific policy |

There is no public `PresentationTheme`, `HostTheme`, `PresentationProfile`, or aggregate
`Presentation` replacement. Product recipes belong in the host application and compose these
owners explicitly.

## Rust API

Select a built-in typed preset and attach the compiled value to the SVG request:

```rust
use merman::svg::{DiagramThemeCompiler, SvgPipeline, SvgRenderOptions, ThemePreset};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

let theme = DiagramThemeCompiler::new().compile_preset(ThemePreset::OneDark)?;
let output = Renderer::new().render(
    RenderRequest::svg(
        source,
        OperationControl::new(),
        SvgRequest {
            pipeline: Some(SvgPipeline::resvg_safe()),
            options: SvgRenderOptions {
                diagram_id: Some("theme-preset-example".to_string()),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .with_theme(theme),
)?;
let RenderOutput::Svg(Some(svg)) = output else {
    return Err("no Mermaid diagram detected".into());
};
# let _ = svg;
# Ok::<(), Box<dyn std::error::Error>>(())
```

`ThemePreset` currently contains `editor-light`, `editor-dark`, `one-dark`, `gruvbox-light`,
`gruvbox-dark`, `ayu-light`, and `ayu-dark`. Presets are visual theme data. They do not select
`look: neo`, an ELK renderer, an SVG pipeline, or a product behavior profile. The preset's
Mermaid compatibility values are limited to the explicit compatibility lane owned by the theme.

Use the versioned authoring contract for compact cross-family tokens. The materializer expands one
definition into a complete editable spec; the ordinary compiler remains the only semantic compiler:

```rust
use merman::diagram_theme::{
    DiagramThemeCompiler, ThemeColorTokenV1, ThemeDefinitionV1, ThemeTokensV1,
    compile_theme_definition,
};

let definition = ThemeDefinitionV1::new(
    ThemeTokensV1::default()
        .with_color(ThemeColorTokenV1::Canvas, "#0f172a")
        .with_color(ThemeColorTokenV1::Surface, "#111827")
        .with_color(ThemeColorTokenV1::SurfaceAlt, "#1f2937")
        .with_color(ThemeColorTokenV1::Text, "#e5e7eb")
        .with_color(ThemeColorTokenV1::Border, "#475569")
        .with_color(ThemeColorTokenV1::Line, "#94a3b8")
        .with_series(["#60a5fa", "#34d399", "#f59e0b"].map(str::to_owned).to_vec()),
);
let theme = compile_theme_definition(&DiagramThemeCompiler::new(), &definition)?;
# let request = merman::RenderRequest::svg(
#     "flowchart TD\nA --> B",
#     merman::OperationControl::new(),
#     merman::SvgRequest::default(),
# ).with_theme(theme);
```

`merman::diagram_theme` selectively re-exports the dependency-neutral authoring wire used by the
renderer. The name is intentionally visual: terminal/ASCII styling is not part of this contract.
`ThemeMaterializer` remains available when callers need to inspect or edit the complete materialized
spec. It owns defaults, expansion order, palette replacement, and the closed
`MaterializedThemeWireV1` success value; the facade above delegates to that same implementation
before compilation. Version 1 intentionally publishes no materialization digest. The materializer
does not inspect family capabilities or render output. Raw untrusted JSON still requires the bounded
`compile_theme_definition_json` entry point rather than ordinary Serde decoding. Render-time
bindings remain limited to either a preset reference or a complete `theme.spec` until their
authoring operations adopt the same contract.
Built-in preset recipes remain alpha inventory: this migration preserves their resolved visual
winners but intentionally does not freeze prior recipe fingerprints or rule indices.

## Capability Discovery

Static discovery describes the current build's coarse upper bound without claiming that a concrete
document applied a rule or produced a portable output:

```rust
use merman::diagram_theme::{
    DiagramFamilyId, ThemeRuleFacetV1, ThemeSupportOutputV1, ThemeSupportQueryV1,
    ThemeTarget, describe_theme_support,
};

let support = describe_theme_support(&ThemeSupportQueryV1::known(
    DiagramFamilyId::FLOWCHART.as_str(),
    ThemeSupportOutputV1::StandaloneSvg,
    ThemeTarget::Node.id(),
    ThemeRuleFacetV1::Radius,
));

println!("{:?}: {:?}", support.state(), support.reason_ids());
```

The result is one of `Unconditional`, `Conditional`, `NotApplicable`, `Unsupported`, or
`Unverified`. A family-owned direct route and a legacy Mermaid compatibility route may both be
`Conditional`; stable reason IDs distinguish them. Unknown identifiers remain visible and return
`Unverified`. The current alpha implementation makes positive static claims only for Standalone
SVG. Browser SVG and PNG/JPEG/PDF remain `Unverified` until their terminal or export owner projects
qualification into this contract. Only the evidence and target-admission receipt from an actual
render can report application, residuals, host dependence, or portability.

`DiagramThemeSpec` can also be assembled directly. Its typed sections are Mermaid compatibility,
typography, semantic rules and ordinal palettes, canvas, effects, resource assets, and declared
requirements. The compiler validates the complete recipe and returns a `DiagramTheme`; it does not
read CSS variables, selectors, or host application state.

## Independent Configuration

The useful parts of a product recipe that used to be bundled under a named profile are explicit:

```rust
use merman::svg::{DiagramThemeCompiler, ThemePreset};
use serde_json::json;

let theme = DiagramThemeCompiler::new().compile_preset(ThemePreset::OneDark)?;
let engine = merman::Engine::new().with_site_config(merman::MermaidConfig::from_value(json!({
    "look": "neo",
    "flowchart": { "defaultRenderer": "elk" }
})));
let renderer = merman::Renderer::new().with_engine(engine);
let request = merman::RenderRequest::svg(
    "flowchart TD\nA --> B",
    merman::OperationControl::new(),
    merman::SvgRequest::default(),
).with_theme(theme);
# let _ = (renderer, request);
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
| Cross-family tokens and ordered authored rules | `ThemeDefinitionV1` / `ThemeMaterializer` | Experimental versioned authoring |
| Semantic palette, typography, geometry, and ordinal series | `DiagramThemeSpecWireV1` / `DiagramThemeSpec` | Experimental typed support |
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

Bindings use Options JSON schema 3. The current theme field is top-level and is a closed tagged
union: exactly one of `preset` or `spec` must be present. An empty object, both members, or a null
member is invalid.

```json
{
  "version": 3,
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

Typed effect graphs describe only the effect identity and ordered primitives. They do not accept an
authored filter region: the family adapter derives the terminal region from the actual painted
geometry, then applies the effective session resource ceiling before emission. This keeps recipe
identity independent of node size while preventing a stale or undersized authored box from
clipping the output.

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
`default`). Merman's native inputs are separate: render/batch commands use `--theme-definition`
for a shareable `ThemeDefinitionV1`, `--theme-preset` for a built-in preset, or `--theme-file` for
an advanced complete selection. Binding callers use the corresponding Rust-owned theme operation;
no host reimplements token expansion.

The removed `presentation`, `host_theme`, `PresentationTheme`, `HostTheme`, and
`PresentationProfile::MermanModern` names are not compatibility aliases. Options JSON rejects the
removed groups with migration-oriented errors; choose top-level `theme`, `site_config`, explicit
layout configuration, and `svg` instead.
