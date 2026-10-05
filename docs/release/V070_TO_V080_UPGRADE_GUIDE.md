# Upgrading from 0.7.0 to the 0.8.0 candidate

This guide compares the stable `v0.7.0` tag, published on 2026-06-09, with the source candidate for the next stable release, `0.8.0`. The latest published workspace release at this checkpoint is `0.8.0-alpha.7` (2026-09-30); this guide does not announce a `0.8.0` publication. The [comparison report](V070_TO_V080_RELEASE_REPORT.md) records measured revisions, evidence, and remaining release checks. The approximately four-month interval includes breaking API and default changes; upgrading the version alone is insufficient for custom integrations.

## Rust applications

The workspace still requires Rust 1.95 and edition 2024, as v0.7.0 did. The compatibility breaks below concern APIs and dependency selection, not a newly raised compiler floor.

| Previous integration | Candidate action |
| --- | --- |
| Parser-only default `merman` dependency | Disable defaults and select `all-diagrams` or the required `diagram-*` features, or use `merman-core` with an explicit family selection. Facade defaults now build a complete SVG product. |
| `render`, `raster`, `ratex-math` features | Use `svg`; independently select `png`, `jpeg`, or `pdf`; use `math` for mathematical labels. Select languages separately. |
| `merman::render::HeadlessRenderer` | Use `Renderer::render`, `RenderRequest`, and the target-local request type. Move reusable parser configuration to `Engine` and per-output settings to the request. |
| `HeadlessAsciiRenderer` or source-to-raster convenience methods | Use the corresponding operation request and typed output artifact; inspect the target's local capabilities. |
| Vendored font metrics or browser-sized snapshot assumptions | Use deterministic measurement or an explicit host text-measurement provider; review output with the final installed font. |
| Narrow feature declaration relying on transitive defaults | Forward the necessary diagram and output features; verify the actual application graph in an independent workspace. |
| Family-specific model imports and exhaustive enum matches | Enable the owner family and review conditional variants and the new operation-scoped semantic/layout types. |
| `large-features`, or intermediate alpha `full` / `tiny` / `core-full` presets | Remove the historical bundle and choose languages, operations, and optional backends explicitly. |
| `chrono::NaiveDate` passed to `Engine::with_fixed_today` | Use `merman::time::CivilDate::new(year, month, day)`; other public date/time values use `CivilDateTime`, `UtcOffset`, and `OffsetDateTime`. |
| Implicit local clock, time zone, or randomness | The Rust runtime now defaults to deterministic values. Compile the required `system-*` adapters and explicitly select their runtime policy when host behavior is required. |
| `viewport_width` / `viewport_height`, struct-literal layout options, or unbranded export SVG | Use `container_width` / `container_height`, start non-exhaustive options from defaults/builders, and retain the typed SVG artifact expected by the exporter. |

Start with the [Rust embedding guide](../rendering/RUST_EMBEDDING.md) for a runnable request recipe, the Zed-style family selection, layout fallback rules, fonts, and output handling. Use [Features](../FEATURES.md) for the full capability matrix and the [alpha.6 symbol reference](UNRELEASED_UPGRADE_GUIDE.md) for exact replacements introduced during the refactor. That historical reference does not supersede later alpha.7 schema changes.

`layout-cytoscape` and `layout-elk` still imply SVG, but they do not imply diagram families. Use `all-diagrams, svg, layout-cytoscape` to preserve the old broad parser surface of an SVG/Cytoscape embedder. An explicit allowlist can replace `all-diagrams` with its selectors. The complete default includes ELK and math; choosing `all-diagrams, complete-svg` with defaults disabled omits ELK while retaining SVG, Cytoscape, and math. Review the actual resolved graph and include the notices for the artifact being distributed.

### Choose the dependency by purpose

These are source-candidate examples for a consumer checked out beside Merman. Use the published version only after that channel releases 0.8.0. Do not enable every new crate or feature just to repair one missing import.

```toml
# Parser-only, retaining the broad language surface of the old default.
merman = { path = "../merman/crates/merman", default-features = false, features = ["all-diagrams"] }
```

```toml
# SVG for a two-family allowlist; no optional layout engines or math.
merman = { path = "../merman/crates/merman", default-features = false, features = ["diagram-flowchart", "diagram-sequence", "svg"] }
```

For all families with SVG, Cytoscape, and math but without compiled ELK, disable defaults and select both `all-diagrams` and `complete-svg`:

```toml
merman = { path = "../merman/crates/merman", default-features = false, features = ["all-diagrams", "complete-svg"] }
```

Add `layout-elk` only if it belongs in the distributed product. Confirm the final application with `cargo tree -e features -i merman-elk-layered`: Cargo feature unification can re-enable ELK through another dependency. Use a published version instead of the example path only after that version becomes available.

### Understand the crate split

| Need | Public owner | Migration consequence |
| --- | --- | --- |
| Parsing and typed models | Existing `merman-core` | Select diagram families; no renderer is required. |
| Configured rendering and typed outputs | Existing `merman` facade | Usually remain here and migrate requests rather than importing every implementation crate. |
| Lint, diagnostics, Markdown/MDX, semantic facts | New `merman-analysis` | Use a render-free analysis dependency when that is the actual workload. |
| Editor intelligence / a language server | New `merman-editor-core` / `merman-lsp` | Choose embedded editor services or the server; the host keeps document storage, revisions, and scheduling. |
| Direct binary export | New `merman-export` | Owns PNG/JPEG/PDF plans and limits; ordinary consumers can use the facade's output requests. |
| Direct ELK integration | New `merman-layout-elk` / `merman-elk-layered` | Adapter and implementation have different responsibilities and license boundaries; direct struct consumers need the low-level migration. |
| Rustdoc source processing | New `merman-doc`; existing `merman-rustdoc` macro | The shared parser/wrapper crate is generally an implementation detail; select the macro or CLI authoring workflow. |
| Typst integration | New `merman-typst-plugin` bridge | End users select the independently versioned Typst package, not a workspace version as a Typst import. |

The crate split does not itself require changing a facade dependency into multiple low-level dependencies. [Package Surfaces](PACKAGE_SURFACES.md) lists intended entry points; fixture and test-only workspace members are not application products.

## Presentation and snapshots

The source candidate selects Mermaid `12.1.0` at `21f72f07ea22c0af48a3149c550654e80d8e40cb`, compared with `11.15.0` in `0.7.0`; published alpha.7 follows `12.0.0`. Final release checks must match the exact source selected for publication. The [alpha.7-to-0.8.0 guide](ALPHA7_TO_0_8_0_UPGRADE_GUIDE.md) covers feedback-edge orientation, line hops, Packet/XYChart policies, and direct low-level ELK constructor changes. Mermaid 12 introduces ELK defaults for supported graph families, Redux themes, and Neo appearance. Use top-level layout configuration rather than the removed family `defaultRenderer` settings. To retain Dagre and the earlier theme/look choices, put this frontmatter in the diagram:

```text
---
config:
  layout: dagre
  theme: default
  look: classic
---
flowchart TD
  A[Start] --> B[Done]
```

`layout: dagre` selects the runtime layout; `theme` and `look` are optional. It does not remove compiled ELK dependencies or preserve every historical pixel or SVG byte. Use the Cargo declaration above when the distributed artifact must omit ELK.

Refresh SVG IDs, DOM selectors, viewBox/geometry snapshots, fonts, and CSS or resvg checks. Default deterministic text measurement no longer ships vendored font metrics. HTML labels and browser measurement remain separate from the headless path. Review ASCII layout, viewport, encoding, and structured-text fallback metadata independently from SVG support; new SVG families do not automatically gain ASCII support.

## CLI and language tooling

Use the owning `render`, `batch`, and compatibility `mmdc` commands for rendering workflows. The `rustdoc` CLI command remains in the `0.8.0` source default and official CLI archive, as in published alpha.7 binaries. Select it explicitly only for a custom build that disables default features. The separate `merman-rustdoc` proc-macro integration is unchanged. Narrow CLI and LSP builds require a family selection: select `all-diagrams, analysis` for a lint CLI, or `all-diagrams, stdio` for the standalone LSP. Complete CLI defaults now include capabilities that a narrow host may not need; compare the exact recipe rather than the facade's defaults when reviewing installed size.

The refactor adds dedicated analysis and editor owners. Hosts retain document storage, revisions, scheduling, and cancellation; pass caller-owned source text into immutable analysis/editor snapshots. Use exact lowercase profile/severity values, the current configuration schema, and matching analysis-fact decoders. Follow the [alpha.3-to-alpha.5](ALPHA3_TO_ALPHA5_UPGRADE_GUIDE.md), [alpha.5-to-alpha.6](ALPHA5_TO_ALPHA6_UPGRADE_GUIDE.md), and [alpha.6-to-alpha.7](ALPHA6_TO_ALPHA7_UPGRADE_GUIDE.md) guides for those versioned boundaries.

### Update CLI automation

- Prefer explicit `render`, `batch`, or `mmdc`; root `-i/-o` remains a permanent hidden compatibility alias. Native render/batch use `-f/--format`; their hidden `-e` aliases warn in 0.8.x and are scheduled for removal in 0.9.0.
- Read requested bytes from stdout and progress/diagnostics from stderr. Handle exit `2` for invalid input/config/output, `3` for direct I/O, and `1` for runtime/render failure; a broken stdout pipe exits successfully.
- Network icons remain compiled into official builds but require `--allow-network` at execution. Local JSON, `file://`, and local package discovery do not grant network access.
- Decode the final CLI contract and ASCII schemas in the table below. Do not infer ASCII support, layout, or fidelity from SVG family support.

## Bindings and package channels

| Candidate contract inherited from alpha.7 | Upgrade requirement |
| --- | --- |
| C/Flutter ABI 3 | Replace the native artifact and matching facade/header together; reject incompatible ABI probes. |
| UniFFI API 7, including Python and Apple | Regenerate wrappers and replace the matching library or XCFramework together. |
| Android JNI transport API 2 | Replace Kotlin projection and all native slices from one matching AAR. |
| Web/WASM transport API 5 | Keep generated glue, package facade, and WASM on one package version. |
| Options JSON schema 2 | Rename viewport fields to `container_width` / `container_height`; put text/math selection under `environment`, theme values under `presentation.theme`, and use top-level `site_config` / `svg`. |
| Analysis facts schema 2; config schema 2 | Regenerate strict facts/config decoders; editor syntax comes from Tree-sitter rather than parser-emitted token APIs. |
| CLI contract 5 | Refresh capability consumers; read the current descriptor/digest and output-local operations. |
| ASCII report schema 3 | Update strict decoders, requested/effective layout, Compact-attempt, encoding, semantic-coverage, structured-text fallback, and output-plan records. |
| Independent Tree-sitter and Typst versions | Select their own releases; a workspace version or tag does not update these packages. |

These protocol versions are independent. An unchanged C ABI number does not make a changed JSON payload or generated binding record compatible.

### Apply the package-specific migration

| Surface | Required action | Current guide |
| --- | --- | --- |
| Browser | Replace old capability subpaths and raw `pkg/**` imports with the owning `@mermanjs/web*` package. Use `runtimeCatalog()`, retain/dispose an owned browser text-measurement session, and choose self-contained or navigable DOM admission for a manual mount. | [Web SDK](../../platforms/web/README.md) |
| C/C++ | Replace individual ABI exports with the generated API-table discovery contract; preserve ownership and version checks. | [ABI 3 migration](../bindings/ABI3_MIGRATION.md) |
| Flutter/Dart | Move from plugin registrars / `openMermanLibrary()` to Native Assets and `Merman.open()`. Replace `MermanReusableEngine` with `MermanEngine`, put callbacks in constructor-owned services, and use `close()` instead of `dispose()`. Requires Dart 3.10 / Flutter 3.38; package minima include Android 24, iOS 13, and macOS 11. | [Flutter changelog](../../platforms/flutter/CHANGELOG.md) |
| Apple/Swift | Replace hand-written C bindings and raw callback pointers with matching generated UniFFI records, `MermanEngine(optionsJson:services:)`, and constructor-owned services. Use the source facade and XCFramework from one release; Swift 5.9, iOS 14 / macOS 12 are the current package floors. | [Apple guide](../../platforms/apple/README.md) |
| Android/Kotlin | Replace the C-forwarding JNI library with the direct JNI AAR, including `libmerman_android_jni.so`. Move `MermanReusableEngine` and mutable callbacks to `MermanEngine(optionsJson, services)` and immutable services; keep Kotlin and all native slices together. | [Android changelog](../../platforms/android/CHANGELOG.md) |
| Python | Upgrade the generated package and native library together to UniFFI API 7; direct users retain `MermanOperationRequestV4` and adopt the API-7 probe and current payload decoders. | [Python changelog](../../platforms/python/merman/CHANGELOG.md) |
| Node.js | This package group was introduced during the 0.8 alphas, so there is no 0.7.0 Node-package migration. On Node 22+, choose native `@mermanjs/node` or explicit `@mermanjs/node-wasm`; keep the selected group version aligned. The recipe is deterministic SVG/layout and does not include analysis, ASCII, math, or binary exports. | [Node guide](../../platforms/node/README.md) |
| Typst | Use the Typst package's own numeric version. Direct ID options override profile aliases; direct `site-config` replaces the profile object, and raw `options` bypasses shorthand validation. The plugin enforces its constrained policy. | [Typst guide](../../distribution/typst/merman/README.md) |

Default Android, Apple, Python, and Flutter artifacts include SVG, Cytoscape/ELK, ASCII, analysis, validation, and document analysis. They omit math, PNG/JPEG/PDF, and native clock/time-zone/random adapters. Select a matching custom artifact when those capabilities are needed; a helper method's existence does not prove its backend is compiled.

Query the installed artifact's runtime catalog before enabling optional operations. The current native package recipes and browser/Node packages have different capability sets; use [Package Surfaces](PACKAGE_SURFACES.md) rather than inferring them from Rust defaults. Publication is independent for each channel. Keep using the verified published version until the owning registry or release asset confirms `0.8.0` availability.

## Validate the upgrade

Build the actual dependency declaration outside the Merman workspace, then exercise every host-accepted family with the selected layout and output. Verify explicit rejection of an omitted family, cancellation, resource limits, and unavailable or denied backends. Run the final CSS/font/raster/DOM path and refresh snapshots only after reviewing output changes. Bind performance and size reports to the final reviewed source commit; rerun them if production code, compiler, profile, features, or lockfile changes.

The reusable theme refactor remains deferred beyond this release. The stable candidate's theme compatibility and targeted correctness fixes should not be read as completion of that separate design effort. The [comparison report](V070_TO_V080_RELEASE_REPORT.md) keeps product growth, comparable measurements, and unmeasured surfaces distinct.
