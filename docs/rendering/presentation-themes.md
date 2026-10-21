# Presentation Themes and Output

Merman keeps four independent choices separate: host theme values, Merman presentation behavior, Mermaid configuration, and SVG output policy. A caller can combine them, but selecting one never silently selects another.

| Owner | Public input | Use it for |
| --- | --- | --- |
| Host theme | `Presentation::with_theme(...)` / `presentation.theme` | Semantic host colors, typography, and series colors |
| Merman presentation | `Presentation::with_profile(...)` / `presentation.profile` | Product-owned behavior such as the `merman-modern` Flowchart treatment |
| Mermaid configuration | `Engine::with_site_config(...)` / top-level `site_config` | Mermaid `theme`, `look`, layout, `themeVariables`, and family configuration |
| SVG output | `SvgRequest.pipeline` / `svg` | Parity, readable, or `resvg-safe` post-processing and output-specific policy |

The default renderer remains Mermaid-parity oriented. An empty presentation is a no-op. The host
resolves its own appearance and maps those values into the chosen output's theme input; Merman does
not inspect a terminal or editor theme. See the
[host integration recipes](../../crates/merman/examples/README.md#host-integration-recipes) for
executable request composition.

## Rust API

```rust
use merman::svg::{
    HostTheme, HostThemePreset, Presentation, PresentationProfile, SvgPipeline, SvgRenderOptions,
};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

let presentation = Presentation::new()
    .with_profile(PresentationProfile::MermanModern)
    .with_theme(HostTheme::from_preset(HostThemePreset::OneDark));

let resolved = presentation.resolve();
let renderer = Renderer::new().with_engine(
    resolved.materialize_engine(merman::Engine::new()),
);
let output = renderer.render(RenderRequest::svg(
    source,
    OperationControl::new(),
    SvgRequest {
        presentation: resolved.render_policy(),
        pipeline: Some(SvgPipeline::resvg_safe()),
        options: SvgRenderOptions {
            diagram_id: Some("preview".to_string()),
            ..Default::default()
        },
        ..Default::default()
    },
))?;
let RenderOutput::Svg(Some(svg)) = output else {
    return Err("no Mermaid diagram detected".into());
};
# let _ = svg;
# Ok::<(), Box<dyn std::error::Error>>(())
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

See [`presentation_profile.rs`](../../crates/merman/examples/presentation_profile.rs) for a ready-made profile and preset combination, or [`custom_presentation_theme.rs`](../../crates/merman/examples/custom_presentation_theme.rs) for a self-contained semantic-role mapping.

Bundled theme IDs are `editor-light`, `editor-dark`, `one-dark`, `gruvbox-light`, `gruvbox-dark`, `ayu-light`, and `ayu-dark`. The One Dark, Gruvbox, and Ayu presets are Merman's semantic mappings inspired by those color systems; they are not claims of byte-for-byte identity with a particular editor distribution.

Bundled presets keep normal and subtle text at a minimum 4.5:1 contrast against their canvas and structural line colors at a minimum 3:1. Palette labels independently choose black or white by the higher WCAG contrast ratio. Sequence actor and label-box variables follow the actor-specific roles, while Gantt done and critical tasks keep a readable neutral fill and express state through their semantic border color.

Host themes also reach C4 boundary labels, relationships, and arrowheads; Wardley anchor labels and component surfaces; and ZenUML frame, participant, message, note, group, activation, and text colors. Filled C4 boundaries paint behind their children. Gantt active-critical and done-critical tasks retain the active/done fill with a critical border, and outside labels use the surrounding canvas text color. Explicit family configuration and source styles retain priority.

`merman-modern` is a presentation profile, not a theme preset. It selects Redux/slate Mermaid defaults, Neo look, an ELK default for ordinary Flowcharts, and Merman-owned Flowchart SVG behavior. The profile selects `flowchart.layout: elk`; an explicit scoped `layout: dagre` overrides that aspect. In a build without `layout-elk`, Flowcharts render through Mermaid 12's Dagre fallback while the SVG plan reports the ELK presentation aspect as blocked and the operation as ready. A host policy that denies a compiled ELK backend still blocks the operation.

## Terminal themes

ASCII/Unicode uses `AsciiTerminalPalette` and `AsciiColorTheme`, independently of SVG `HostTheme`
and `Presentation`. A host may map the same application colors into both inputs, but SVG typography,
CSS-oriented family variables, and terminal role encoding remain separate. `presentation.theme`
does not populate `ascii.theme` or select an ASCII color mode.

| Host intent | ASCII color mode | Palette behavior |
| --- | --- | --- |
| Plain logs or a machine text channel | `Plain` | No color escapes; viewport fallback is available |
| Use the terminal's own palette | `Ansi16` | Primary text and most structural roles use Reset; emphasis uses named ANSI colors |
| Apply a known host RGB palette | `TrueColor` | Semantic roles resolve through `AsciiColorTheme` |
| Approximate a host RGB palette | `Ansi256` | Resolved RGB colors are mapped to the 256-color encoding |

ANSI16 deliberately bypasses the RGB theme for semantic roles; direct authored RGB colors are
quantized separately. Named terminal colors and Reset do not guarantee contrast for an arbitrary
user-customized terminal palette. Styled output accepts viewport `Allow` or `Error`; `Fallback` is
Plain-only and an invalid styled/fallback combination is rejected even when the diagram would fit.

See the [terminal theme API](../../crates/merman-ascii/README.md#terminal-theme-api) and
[binding ASCII options](../bindings/OPTIONS_JSON.md#ascii-options) for palette fields and overrides.

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

Configuration resolves from defaults to explicit author choices:

1. The pinned Mermaid defaults.
2. Defaults selected by `presentation.profile`.
3. Host defaults from `presentation.theme`.
4. Explicit `Engine::with_site_config(...)` or top-level `site_config` values.
5. Allowed diagram frontmatter and directives.

Rust callers materialize presentation defaults into an `Engine`, apply explicit site configuration
to that engine, and pass the resolved render policy through `SvgRequest.presentation`. Bindings
perform the same structural merge from their separate `presentation` and `site_config` fields.
Explicit Mermaid configuration and source-local configuration win over presentation defaults.
The independently selected SVG output pipeline runs after rendering.

Selecting a Mermaid `theme` in site configuration, frontmatter, or a directive selects its complete derived palette, together with raw explicit site/source variables. Merman does not carry host-derived colors from the previous palette into values that the selected theme defines. If the selected theme lacks a family paint value, its corresponding host role remains a fallback. A single explicit `themeVariables` value retains Mermaid's property-level derivation rules and does not replace the whole palette.

Typography is independent: host font family and size defaults survive a palette selection unless explicit site/source typography overrides them. For example, `presentation.theme.preset: editor-dark` with `site_config.theme: forest` uses Forest's matching foregrounds and backgrounds while retaining an independently supplied host font. Source-local family styles remain the final owner of their own properties.

Palette, font-selection, and family paint repairs can change SVG and exported image/PDF snapshots. Review regenerated output when updating within 0.9.x; the existing `Presentation`/`HostTheme` API and Options JSON schema 2 continue to apply.

## Discovery

Rust callers enumerate built-in values through `theme_preset_descriptors()` and `presentation_profile_descriptors()`. Native and Web callers use the `presentation-catalog` metadata payload. Catalog IDs are open strings: consumers must tolerate future presets, profiles, aspect kinds, and capability requirements instead of treating the current list as a closed enum.

The catalog reports artifact-level availability. `svg-plan-json` reports the operation-specific result after the diagram family, effective renderer, explicit overrides, and compiled capabilities are known.
