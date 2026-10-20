# Merman 0.8.0 to 0.9.0 upgrade guide

This guide covers upgrading from Merman 0.8.0 to 0.9.0. It describes changes to Rust APIs, CLI commands, rendered output, bindings, and independently versioned packages. See the [release index](README.md) for earlier upgrade paths.

## Rust dependencies

Upgrade coupled Merman crates together and retain explicit feature selection. Rust 1.95 remains the minimum toolchain. Stable internal workspace requirements use the ordinary compatible `0.9.0` requirement; exact prerelease lockstep rules do not apply to this stable release.

```toml
[dependencies]
merman = "0.9.0"
```

Default facade features still include every built-in diagram family, SVG, Cytoscape, ELK, and math. For smaller builds, use the [feature recipes](../FEATURES.md); selecting Dagre at runtime does not remove compiled ELK. Mermaid remains at 12.1.0, so this upgrade does not introduce another Mermaid major-version migration.

## Rust models and JSON

`ParsedDiagram.model`, parse outcomes, semantic projections, and layout JSON now use `ManagedSemanticJson` in place of owned `serde_json::Value`. Keep the managed owner for iterative clone, equality, destruction, and deep output. Use `as_value()` for inspection; converting with `into_unmanaged_value()` transfers responsibility for recursive serde and destruction to the caller.

Generic serde serialization has a 128-container output boundary. Use the maintained writers for deeper output:

```rust
let mut bytes = Vec::new();
parsed.model.write_json(&mut bytes)?;
```

Use `write_json_pretty` for indented output, or the controlled writers when cancellation and deadlines matter. Their outer error is operation termination; their inner error is JSON serialization or writer I/O. Any partially written bytes remain the caller's responsibility.

| Direct model consumer | Required action |
| --- | --- |
| State | Traverse `StateDocument` through model-owned `StateDocumentId` references. |
| Block | Read `blocks_flat`, `model.block(index)`, and record child indices. |
| Treemap | Resolve root and node child indices through `model.nodes`. |
| Ishikawa | Resolve the optional root index and child indices through `model.nodes`. |
| Nested typed-wire serde on these owning models | Use canonical records in Rust, or `compatibility_json()` for Mermaid-shaped JSON. |

Mermaid-compatible field names, nesting, and ordering remain unchanged. Existing source/model limits and operation termination rules retain their meanings. The change also makes cleanup safe for deeply nested successful, malformed, cancelled, and resource-rejected inputs. See the [complete symbol and ownership reference](../migrations/deep-diagram-lifecycle.md) before changing low-level integrations.

## SVG backgrounds and snapshots

Core SVG and native CLI SVG no longer add an implicit white canvas. Set an explicit background when embedding into a dark page, exporting for a white document, or comparing snapshots that require an opaque canvas:

```sh
merman-cli render diagram.mmd --output diagram.svg --background white
```

Rust callers set `SvgOutputPolicy.root_background_color`. `merman-cli mmdc` keeps its white default. Diagram-owned edge-label colors and opacity retain their established behavior. Review output snapshots for corrected hollow Class/ER markers, Unicode wrapping, and Block metadata updates.

## Structured diagram themes

Rust callers compile a `DiagramTheme` and attach it with `RenderRequest::with_theme(...)`. Mermaid-compatible configuration still belongs to `site_config`; reusable visual styling belongs to the compiled theme. Replace the removed presentation-profile APIs and `presentation-catalog` discovery with the theme APIs and `theme-catalog`.

Bindings advance Options JSON from schema 2 to 3. Move `presentation.theme` to the closed top-level `theme` group. Select an artifact-supported preset, a complete spec, or a version-1 recipe; removed presentation paths are rejected. For example:

```json
{
  "version": 3,
  "theme": { "preset": "editor-dark" }
}
```

Mermaid theme names such as `default`, `dark`, and `forest` remain Mermaid configuration values. Merman presets such as `editor-dark`, `brutalist`, `spotless`, and `cyberpunk` compile reusable styling and do not select a layout engine or SVG pipeline. Discover presets and query support against the loaded artifact: catalog availability does not establish support for every diagram family or output target. Use `raster.matte` and `pdf.page_paint` for export backgrounds. See the [theme authoring guide](../rendering/custom-diagram-themes.md) for customization, canonical JSON sharing, and the preset qualification scope.

## Fonts and CLI discovery

Ordinary CLI commands keep deterministic text measurement without discovering system fonts for measurement. Native-font support is compiled into default/release CLI builds but is opt-in at runtime; Rust and FFI defaults remain host-independent. Explicit fonts enable actual-font measurement and share the selected context with PNG/JPEG/PDF drawing:

```sh
merman-cli render diagram.mmd --output diagram.svg --font-file fonts/FiraSans-Regular.ttf --font-file fonts/FiraSans-Bold.ttf
merman-cli render diagram.mmd --format png --output diagram.png --font-dir fonts
merman-cli render diagram.mmd --output diagram.svg --font-measurement native
```

Set Mermaid `fontFamily` to the actual supplied family. Explicit sources form a closed set unless `--font-system on` is added. Include every used bold/italic face; the CLI bundles no diagram text fonts, and SVG keeps text instead of embedding fonts. Codec painting may still discover fonts through its existing export path. Actual-font measurement does not promise browser pixel identity, and unsupported custom typography requires deterministic mode. See the [font guide](../../crates/merman-cli/README.md#explicit-fonts).

Rust `RenderedDocument` values retain the native font context admitted during layout. PNG/JPEG/PDF exports inherit that context; an explicit override must be the same shared context. Supplying a different context, or adding one after deterministic layout, is rejected because painting cannot change completed text geometry. Select the font context before preparing the document.

CLI capabilities retain outer schema 2 and advance `cli_contract_version` from 5 to 6. Read the native-font policy separately from compiled support.

Native `render -e` and `batch -e` are removed in 0.9.0 as scheduled by 0.8.x. Replace `merman-cli render diagram.mmd -e png` with `merman-cli render diagram.mmd -f png`, and use `-f` / `--format` for `batch` too. The pinned `merman-cli mmdc -e/--outputFormat` interface remains supported.

## Labels and Typst documents

Review snapshots containing long HTML labels, preserved newlines, non-breaking spaces, soft hyphens, Markdown code spans, or escaped tags. Layout and exported SVG now share wrapping and decoded-text semantics. XHTML label paths keep literal angle brackets; Flowchart HTML labels follow Mermaid's raw-token handling. See [PR #192](https://github.com/Latias94/merman/pull/192) and [PR #193](https://github.com/Latias94/merman/pull/193).

Typst `@preview/merman:0.5.0` keeps 0.4.0's rendering functions, figures, profiles, raw-block handlers, options schema 2, plugin ABI 2, and minimum Typst 0.15.0. Update the import to `@preview/merman:0.5.0` and review the [package changelog](../../distribution/typst/merman/CHANGELOG.md). Use `background: "white"` for opaque artwork. ELK defaults and Dagre opt-in recipes already shipped in 0.4.0; this update does not add another default-layout migration or native-font measurement.

## Edge geometry and terminal output

For development builds after `0.9.1`, see the
[later geometry resource migration](../migrations/render-reuse-host-integration.md#migrate-edge-geometry-budgets).
The SVG-admission description below records this guide's historical release contract.

Use the SVG-capable `edge-geometry-json` operation or typed `RenderTarget::EdgeGeometryJson` for clipped points and painted label centers in Flowchart, Swimlane, and Agentflow. `layout-json` still returns pre-paint layout coordinates. Other families return an empty edge list. The operation observes SVG output admission as well as layout-work limits. Web packages with rendering expose `edgeGeometryJson`, `edgeGeometryObject`, and `edgeGeometryJsonWithTextMeasurer`.

For the subsequent removal of SVG generation and migration to an encoded geometry budget, see [Edge geometry resource migration](../migrations/edge-geometry-resources.md).

ASCII/Unicode routing now preserves loop-back contacts, distinct labels, closed compound frames, sibling gutters, and Sequence label spacing. Padding inside Flowchart/Swimlane/State nodes is separate from graph spacing:

```sh
merman-cli render diagram.mmd --format unicode --ascii-node-padding-y 0
```

Rust uses `with_node_padding`, `with_node_padding_x`, and `with_node_padding_y`. Bindings accept `node_padding_x` / `nodePaddingX` and `node_padding_y` / `nodePaddingY`; zero is valid, and axis overrides take precedence over `box_border_padding`. Complete labels may need more grid cells. Resource ceilings are unchanged: author explicit wrapping or select the appropriate bounded resource profile rather than assuming old width snapshots still fit.

## Binding artifacts and callbacks

Keep generated wrappers and native/WASM artifacts from one release. Options JSON advances from schema 2 to 3, and Web transport advances from API 5 to 6 for the structured-theme contract. Native ABI 3, UniFFI API 7, Android JNI API 2, Node transport API 1, and ASCII report schema 3 retain their independent versions. An unchanged transport number does not make arbitrary artifact versions interchangeable; regenerate wrappers with their artifacts and query the installed runtime catalog before using additive operations.

Custom host measurers now receive decoded HTML line-width candidates through `CanvasMeasureTextWidth` in the `Wrap` phase and must return a length. Update callbacks that only handled wrapped-metrics requests; preserve the full candidate text instead of interpreting decoded angle brackets as markup.

Text-measurement protocol 1 adds signed length operations `raw-bbox-y-offset` (19), `raw-middle-bbox-y-offset` (20), and `raw-hanging-bbox-y-offset` (21). Update exhaustive custom callback handlers and preserve signed results; inspect [host measurement](../bindings/HOST_TEXT_MEASUREMENT.md) for result tagging, whitespace, and unsupported-operation behavior. Native CLI font flags do not add native font discovery to browser or default mobile/Python artifacts.

## Independent packages

| Surface | Version and upgrade action |
| --- | --- |
| `roughr-merman` | 0.12.4 updates private host-random dependencies while preserving the public legacy svgtypes 0.11 API. |
| Tree-sitter Mermaid | 0.3.1 uses the paired CLI/Rust/Web 0.27.1 contract. The public language adapter uses compatible `^0.1.9`; generated language ABI remains 15. |
| Typst | `@preview/merman:0.5.0` bundles Merman 0.9.0 and keeps plugin ABI 2. Version 0.4.0 bundles Merman 0.8.0. Theme operations in the local ABI 3 development snapshot are separate from that package baseline. See the [Typst package guide](../../distribution/typst/merman/README.md#upgrading-from-040). |
| VS Code | Extension version 0.1.0 uses Merman 0.9.0 runtime binaries. See the [extension guide](../../tools/vscode-extension/README.md) for installation and runtime selection. |

These packages use independent version numbers. Review the [dependency policy](../DEPENDENCY_POLICY.md) for compatible dependency lines, exact generator groups, and intentional upgrade exceptions.
