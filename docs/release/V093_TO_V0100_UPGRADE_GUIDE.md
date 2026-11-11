# Upgrade from Merman 0.9.3 to 0.10.0

This guide targets the selected **unreleased 0.10.0 stable candidate**. It consolidates the consumer actions since stable `0.9.3`, including changes first published in alpha.1 and alpha.2. Upgrade directly to the final contracts below; the published alpha changelogs remain historical references. Use the [host installation guide](PREBUILT_PRODUCTS.md) for available package versions and the [release index](README.md) for other upgrade paths.

Stable `0.9.3` belongs to the separate `release/0.9.x` maintenance line and retains Mermaid `12.1.0`, the Presentation API and Options JSON schema `2`. The `0.10` line selects Mermaid `12.3.0` and ELK `0.12.1`. Its cumulative narrative compares the final source trees, rather than treating maintenance fixes as new features. Native C/WASI SDKs, engine-local Iconify packs, CLI native-font defaults and the text/formula fixes already shipped in `0.9.3` remain part of the upgrade baseline.

## Replace presentation options and update bindings

Replace presentation-profile selection with the top-level `theme` group. A final Options JSON selection is:

```json
{
  "version": 3,
  "theme": { "preset": "editor", "appearance": "dark" }
}
```

Use `raster.matte` for an image export matte and `pdf.page_paint` for PDF page paint. A theme's canvas is diagram styling; the viewer backing is a separate host choice. Mermaid's base palette remains `site_config.theme`, and explicit source styles/configuration retain their property-specific priority over Merman styling. See the [Options reference](../bindings/OPTIONS_JSON.md) and [theme guide](../rendering/custom-diagram-themes.md).

| Contract | Final candidate version or action |
| --- | --- |
| Binding Options JSON | `3`; replace the Presentation API input grammar |
| Web/WASM transport | `10`; deploy the generated facade, JavaScript glue and WASM together |
| CLI discovery | `7`; consume grouped designs and their declared appearances |
| Analysis facts | `3`; retain source appearance declarations and their original UTF-8 ranges |
| Theme execution evidence | `2`; use `ThemeExecutionEvidenceV2` and its actual materialized Mermaid palette |
| Theme recipe | `1`; optional `appearances` and `default_appearance` live inside `design` |
| Native C ABI and host text-measurement protocol | Retain ABI `3` and protocol `1`; regenerate wrappers with the matching runtime and handle the callback extension below |

Discover themes through the installed runtime's theme catalog rather than the former presentation catalog. Appearance entries expose `concrete_preset_id`; do not construct IDs by adding suffixes. Existing concrete IDs such as `editor-dark` stay fixed to that appearance. Use canonical `editor`, `gruvbox` or `ayu` IDs to choose Light/Dark within one design. Unsupported appearance choices fail rather than recoloring an unrelated recipe.

Build coupled Rust packages, generated wrappers and native/WASM artifacts from the same release. An unchanged ABI or recipe schema does not make wrappers from different releases interchangeable. Public theme availability, dedicated family design and output qualification are separate facts; inspect the installed catalog and each render's evidence.

## Try and edit a theme from source

Run these commands from the selected `0.10.0` source checkout. They install from local source and do not depend on a stable registry package or tag already being published:

```text
cargo install --path crates/merman-cli --locked --features math
merman-cli render docs/rendering/theme-gallery/brutalist.mmd --theme-preset paper-cut -o paper-cut.svg
merman-cli render docs/rendering/theme-gallery/brutalist.mmd --theme-preset carbon-copy -o carbon-copy.svg
merman-cli render docs/rendering/theme-gallery/ghibli.mmd --theme-preset ghibli -o ghibli.svg
merman-cli render docs/rendering/theme-gallery/brutalist.mmd --theme-preset editor --theme-appearance light -o editor-light.svg
merman-cli render docs/rendering/theme-gallery/brutalist.mmd --theme-preset editor --theme-appearance dark -o editor-dark.svg
```

The maintained Rust [preset example](../../crates/merman/examples/theme_preset.rs) defaults to Paper Cut. Its arguments are `[preset-id [light|dark]]`; the examples below render one preset and both Editor appearances:

```text
cargo run --locked -p merman --features svg --example theme_preset > paper-cut.svg
cargo run --locked -p merman --features svg --example theme_preset -- pop-art > pop-art.svg
cargo run --locked -p merman --features svg --example theme_preset -- editor light > editor-light.svg
cargo run --locked -p merman --features svg --example theme_preset -- editor dark > editor-dark.svg
```

In an application, compile a built-in theme with `DiagramThemeCompiler::compile_preset(ThemePreset::PaperCut)`, or select a paired design with `compile_preset_selection("editor", Some(ThemeAppearance::Dark))`. Attach the resulting `DiagramTheme` with `RenderRequest::with_theme(...)`. The example contains the complete imports, operation control, SVG pipeline and output handling.

Run the maintained [recipe example](../../crates/merman/examples/theme_recipe.rs) to export, edit, reimport and render a complete document:

```text
cargo run --locked -p merman --features svg --example theme_recipe -- target/theme-recipe
merman-cli theme inspect target/theme-recipe/theme.json
merman-cli render docs/rendering/theme-gallery/brutalist.mmd --theme-file target/theme-recipe/theme.json -o custom.svg
```

It writes three files under `target/theme-recipe`: `paper-cut.json` is the original exported recipe; `theme.json` changes the shared `surface` color to `#DCEAF7` and labels it **Blue face sheet**; `diagram.svg` renders the edited recipe. Edit token values in the JSON to keep their referenced consumers coordinated. The example runs raw JSON preflight before typed deserialization, then compiles the imported recipe. Keep the complete `schema_version`, `kind` and `design` document rather than copying only its tokens.

Use `export_preset_document("editor")` when exporting a paired design; it preserves both appearances. `export_preset(ThemePreset::PaperCut)` exports the single-appearance Paper Cut recipe, while a concrete ID such as `editor-dark` exports that appearance's document. `theme inspect <file|->` validates the whole portable recipe, including inactive appearances, and prints inspection JSON. Render-selection wrappers are accepted by `render --theme-file`, not by `theme inspect`.

Playground and VS Code provide **Follow app/editor**, **Light** and **Dark**. Hosts resolve Follow before requesting a concrete appearance; CLI and runtime APIs do not read operating-system appearance. Shared edits retain appearance-local exceptions, so edit `design.appearances.dark` when a value should affect only Dark. Do not infer a missing appearance by inverting colors. See [appearance selection](../rendering/custom-diagram-themes.md#choose-an-appearance) and [JSON customization](../rendering/custom-diagram-themes.md#customize-an-existing-preset).

The built-in catalog contains 11 designs and 14 concrete appearances. The Playground library contains 28 designs in a complete current runtime, including all 11 built-in preset groups and standalone Reading and Prism pairs. Presets remain alpha; a reviewed gallery scene or an available catalog entry does not qualify every family, shape or output. Source styles, finite palettes and host fonts can change the result. Use the [visual showcase](../../CHANGELOG.md#designed-themes-ready-to-customize), [coverage guide](../rendering/diagram-theme-coverage.md) and [font guidance](../rendering/custom-diagram-themes.md#fonts-across-hosts) when adopting a design.

## Update Rust integrations

The high-level `merman` facade remains the source-to-output entry point. Applications that already own models can select `merman-model`, `merman-ascii`, `merman-render` and `merman-export` independently. Lower-level packages have empty default features; select diagram families and output capabilities at their owners. Use the [model/source migration](../migrations/model-source-boundaries.md) for the complete import and package mapping.

| Existing integration | Required action |
| --- | --- |
| Family data imported from Core or pure `*RenderModel` aliases | Import canonical types from `merman_model::families` or `merman::model::families`; actual presentation carriers retain their distinct purpose |
| Semantic inspection mixed with later graphical output | Use `SemanticParser`, `Renderer::parse_semantic` or `RenderRequest::semantic` for canonical inspection; request `prepare_presentation` / `RenderRequest::presentation` for full graphical source processing |
| Full engine site configuration | Pass `SourcePresentationConfig::from_config(config)` to `MermaidEngine`; semantic parsers continue to accept `MermaidConfig` |
| Low-level graphical preparation | Pass a borrowed `PresentationDiagram` and keep its owner alive for `FamilyRenderArtifact`; preserve output-specific context instead of reconstructing it from canonical data |
| Compatibility JSON or editor captures | Select `compat-json`, `presentation` and `editor-facts` as required; canonical analysis/editor facts do not automatically validate visual theme programs |
| Diagnostic fixes | Enable `fixes` and request `FixGeneration::Generate`; the default is `Omit`, including in a fixes-capable build |
| Direct Flowchart construction | Add `FlowEdge::step: None` when no step is authored; initialize `FlowchartModel::dropped_subgraph_members` to an empty collection and its `subgraph_vertex_styles` provenance to `Default::default()` for ordinary manual models |
| Direct Mindmap construction | Use `PresentationContext::None`, with diagram identity supplied through SVG options rather than compatibility JSON |
| Direct Rectpacking `Options` literals | Add `regular_packing_minimum: None`, or retain `..Default::default()` when no regular-pipeline title floor is needed |
| Journey, Quadrant, Block, Packet, Radar, C4 and Wardley numeric records | Update integer fields to their final `f64` forms; preserve JavaScript Number behavior and use `C4RelationOffset` for numeric versus named string offsets |
| Direct Sequence layout consumption | Replace `SequenceBlockLayout` / `block_layouts_by_id` with `SequenceDiagramLayout.frames` and each frame's `sections`; initialize the additional drawing fields in direct layout literals |
| Family typography queries | `TypographySpec::family_style()` returns an owned, resolved `TextStyle`; bind it before borrowing from it |

For example, direct Journey literals now use `score: 4.0`, Quadrant radii use `Some(5.0)`, and ordinary numeric C4 relation offsets use `Some(C4RelationOffset::Number(10.0))`. Block edges add thickness and pattern fields; use their normal/solid defaults where appropriate. The [numeric migration](../migrations/mermaid-12-3.md#rust-source-changes) lists exact fields, nonfinite canonical JSON forms and the unchanged Mermaid compatibility projection. Regenerate persisted canonical records from source when possible; preserve numeric-versus-string C4 offsets rather than flattening them into strings.

Sequence geometry now includes final control frames/dividers, activation rectangles, participant boxes and positioned titles for native painters. SVG consumes the same prepared geometry; custom painters still own fonts and painting. See the [Sequence drawing contract](../rendering/RUST_EMBEDDING.md#consume-sequence-drawing-geometry-in-development-builds). The [embedding guide](../rendering/RUST_EMBEDDING.md) owns runnable Rust integration recipes.

Facade crates can re-export the Rustdoc attribute with `crate_path`, provided they re-export both `merman` and `__render_doc`. See the [Rustdoc facade example](../../crates/merman-rustdoc/README.md#re-exporting-the-attribute).

## Select math and font services explicitly

Rust source defaults and the retained `complete-svg` / `complete-svg-elk` aggregate features no longer enable math. Add `math` explicitly to retain formula support, for example `features = ["complete-svg-elk", "math"]`. With default features disabled, also select `all-diagrams` or the required `diagram-*` features. Source-built CLI installations add `--features math`, as above. Ordinary prebuilt capability recipes select their own math support; inspect the artifact rather than inferring it from the workspace version.

`merman-export` features `png`, `jpeg` and `pdf` no longer enable system font discovery implicitly. Select `system-fonts`, supply `NativeFontContext::from_font_bytes(...)`, or choose the optional `bundled-fonts` Cousine fallback for supported text. Match layout and painting fonts for image/PDF exports; SVG viewers also need the named fonts. Native font measurement can improve geometry for the host's fonts but adds font discovery/loading and shaping work. Choose it when that geometry matters, using the [host text guide](../bindings/HOST_TEXT_MEASUREMENT.md) and measured workflows rather than assuming a universal performance tradeoff.

The CLI native-font defaults already shipped in `0.9.3`. Upgrading does not require undoing that migration. Explicit CSS admission, image/PDF painting and deterministic SVG measurement remain separate choices; consult the [CLI font guide](../../crates/merman-cli/README.md#explicit-fonts) before changing them.

## Update text measurement and resource policies

Custom text-measurement adapters now receive `html-line-height` operation `22`, whose result kind is `length`. Handle it or retain the unhandled-operation fallback; Rust exhaustive matches require an `HtmlLineHeight` arm. Protocol `1` is unchanged. Stable `0.9.3` had only operations `0..21`, so existing adapters should be reviewed even when their transport version still matches. See the [callback summary](../bindings/HOST_TEXT_MEASUREMENT.md#text-measurement-protocol-summary).

For `edge-geometry-json`, replace operation-local `max_svg_bytes` with `max_geometry_json_bytes` when bounding encoded output. SVG-element and native SVG-backend overrides are no longer meaningful for that query and are rejected. The geometry payload remains schema `1`; source occurrence order, painted routes and label centers remain its contract. Rust callers use `EdgePaintGeometryOutput::write_json_controlled` for bounded encoding. See the [geometry resource migration](../migrations/edge-geometry-resources.md).

Constrained policy now rejects formulas even when the artifact includes math. Handle that rejection, or select an appropriate policy for permitted workloads on hosts that expose one, retaining host execution and memory limits. Compiling math does not override resource admission. The published `0.9.x` maintenance policy is unchanged; consult the [SDK policy guide](../../distribution/sdk/README.md#native-c-sdk).

Matching development Typst wrapper/plugin pairs use **14,100,000 layout work units** by default for SVG, taking the shared interactive allowance while retaining other constrained ceilings. Explicit finite `resources.limits.max_layout_work_units` values may lower or raise that allowance. An explicit `constrained` profile retains **125,000** unless overridden. Published Typst wrapper **0.5.0**, which bundles Merman **0.9.0**, is unchanged; these development changes do not establish a new Typst registry release. See the [Typst README](../../distribution/typst/merman/README.md) and [issue #300](https://github.com/Latias94/merman/issues/300).

## Reuse prepared source and review output changes

Use Rust `Renderer::prepare_source` / `render_prepared` or Web `prepareSource` to reuse parsed source and configuration for several outputs. Supply fresh operation control per output and dispose Web prepared wrappers when done. Preparation freezes source, configuration, theme and parse-time context; it does not cache completed layout or bypass quotas. Reprepare when those inputs change. See the maintained [Rust redraw example](../../crates/merman/examples/redraw_prepared.rs) and [Web lifecycle examples](../../platforms/web/README.md#maintain-a-mounted-diagram).

Web `mountDiagram` owns resize/font refresh, retained snapshots and cleanup. Select `textMeasurement: "browser"` when layout needs the page's actual font metrics; otherwise deterministic measurement remains the default. Kanban `layout.adaptation: "fit-width"` is explicit column packing, separate from SVG viewport scaling; other families reject that adaptation. Failed updates retain the last successful diagram.

Review snapshots after adopting Mermaid `12.3.0`, ELK `0.12.1`, corrected label measurements and source/configuration precedence. Flowchart Step groups support browser motion and reduced-motion handling; static SVG and PNG/JPEG/PDF retain ordinary edges and numbered badges without animation. Nine `bpmn-*` shapes extend Flowchart syntax rather than adding a BPMN language. Header-only Class programs now reject; an empty diagram needs a statement:

```text
classDiagram
direction TB
```

The [Mermaid migration](../migrations/mermaid-12-3.md), [ELK scope](../../crates/merman-elk-layered/MERMAID_12_3_DELTA.md) and [SVG pipeline](../rendering/SVG_OUTPUT_PIPELINE.md) describe the selected behavior. Font rendering, HTML labels, browser measurements and hand-drawn output retain bounded differences; the release does not promise pixel identity or transfer historical benchmark results to every host.

## Upgrading from an alpha

| Installed version | Remaining actions |
| --- | --- |
| `0.10.0-alpha.1` | Adopt Mermaid 12.3/ELK 0.12.1 and the final model/layout changes; update facts to schema `3` and theme evidence to schema `2`; then apply the appearance and wrapper actions below |
| `0.10.0-alpha.2` | Update Web transport `9` to `10` and CLI discovery `6` to `7`; consume grouped designs and `concrete_preset_id`; apply paired appearance, numeric model, Sequence geometry and typography changes |

Recipe schema remains `1`. Keep appearance definitions under `design`, export the whole pair when needed, validate every branch before selection, and preserve concrete preset IDs as pinned choices. Family typography now inherits each omitted property from the shared default. Raw design materialization uses the same semantic/effect admission as recipe compilation; `ThemeDesignCompileError::Materialization` carries editable-design diagnostics, while `complete_spec` compilation retains `Compilation`.

VS Code preserves theme canvas paint by default. Its viewer background controls preview backing; explicit export canvas overrides remain available. Imported themes are panel-local snapshots, while configured files resolve against the current source workspace. Rebuild matching adapters and runtimes rather than using the stable guide's final contract numbers with an older alpha binary.

## Package and channel choices

Use the [host guide](PREBUILT_PRODUCTS.md) for installable CLI, Web, Node, Apple, Android, Python, Flutter and SDK versions and their actual capabilities. Platform archives, registry packages and remote package repositories have separate delivery owners. This guide documents the selected candidate APIs and source workflows; [Publish Order](PUBLISH_ORDER.md) records release qualification and publication evidence.

Tree-sitter and Typst remain independently versioned. The Tree-sitter `0.3.4` patch is being prepared after published `0.3.3`; use a matching grammar and the LSP's declared minimum, and check the [grammar release guide](TREE_SITTER_MERMAID.md) for delivery. Roughr remains `0.12.6`. Published Typst `0.5.0` does not acquire new theme APIs merely because the workspace advances.
