# Changelog

All notable changes to this project will be documented in this file.

The format is based on *Keep a Changelog*, and this project adheres to *Semantic Versioning*.

## [Unreleased]

- Added a host text-measurement callback to the C FFI reusable engine API and bumped the C ABI to
  version 2. This addresses a class of headless rendering issues where the browser or native host
  chooses a different font fallback than merman can know ahead of time, such as Flowchart labels
  clipping trailing characters in some browser/font combinations. Merman now keeps Flowchart HTML
  labels non-clipping by default and lets precise hosts supply their own DOM/canvas/native text
  measurements through the callback.
- Extended the Android JNI, Apple Swift, and Flutter/Dart FFI wrappers to expose reusable engines
  and host text-measurement callbacks on top of the C ABI v2 contract.
- Improved `dugong` layered layout performance by avoiding repeated dummy-node ID scans during
  long-edge normalization and by using cached adjacency when removing graph nodes. Added a
  `layout_dagreish` benchmark covering the full layout pipeline and normalize run/undo costs.
- Improved benchmark coverage and hotspot workflow for Mermaid parity work. Added a corpus-backed
  perf runner, shared corpus helpers, contract tests, and a profiler-friendly `profile_render`
  example; Mermaid JS benchmarking now loads ZenUML on demand. Updated the performance docs to use
  the new canary/default execution order and the broader stage/stress suite naming.
- Reduced architecture and XYChart hot-path overhead by trimming repeated string formatting,
  avoiding extra text-measurement work, and shrinking SVG allocation churn in the parity renderers.
  Against Mermaid JS, the latest cross-family geomean is `0.016x` of Mermaid JS time, which is
  about `62x` faster overall. `xychart_medium` is `73.62 µs` end-to-end versus `3.00 ms` in
  Mermaid JS, or about `41x` faster.

## [0.8.0-alpha.2] - 2026-06-13

This alpha continues Merman for the next stable release by tightening headless rendering
consistency, improving Mermaid parity gates, and paying down renderer architecture debt. Most of
the work is internal, but it should make CLI, library, SVG, and raster output behave more
predictably across diagram families.

### Added

- Added an opt-in `cytoscape-layout` feature for Architecture and Mindmap, so size-sensitive
  browser and Typst builds can leave those heavier layout dependencies out. The wasm-size matrix
  now reports browser, Typst, full, stripped, gzip, and brotli presets for repeatable release gates.
- Added Homebrew install guidance for `merman-cli` on macOS and Linux. Thanks
  [@colindean](https://github.com/colindean) for the contribution in
  [#4](https://github.com/Latias94/merman/pull/4).

### Changed

- Unified SVG-to-raster export through the same renderer-owned operation pipeline used by
  library and CLI callers, so sanitization, sizing, and encoding now follow one path.
- Centralized diagram configuration handling across the renderer. Family-owned config views now
  preserve Mermaid-compatible defaults while keeping layout and SVG decisions closer to the code
  that consumes them.
- Improved C4 and Journey layout parity by separating their configuration/layout internals and
  refreshing the affected layout baselines.
- Tightened admission checks so parser, layout, and SVG coverage claims are validated against
  `merman-core` diagram-family capability facts before alignment reports pass.
- Simplified parser and SVG parity internals behind focused modules, including the core parse
  pipeline, family renderers, presentation themes, and CSS regression tests.

### Fixed

- Fixed Journey raster rendering in the resvg pipeline. Thanks
  [@vlasky](https://github.com/vlasky) for the contribution in
  [#6](https://github.com/Latias94/merman/pull/6).
- Refreshed Kanban and Timeline layout snapshots to restore CI.

## [0.8.0-alpha.1] - 2026-06-10

This alpha starts the 0.8 line with a smaller, clearer feature surface and a real Typst package path. The default Rust crate behavior remains Mermaid-compatible, while no-default and Typst-oriented builds can now avoid host-only and full-config dependencies.

### Added

- Added an experimental `merman-typst-plugin` WebAssembly bridge and local Typst package surface. The package supports `#mermaid(...)` for embedded SVG images, `#show raw.where(lang: "mermaid"): show-mermaid-blocks(...)` for Mermaid fenced code blocks, `mermaid-svg(...)` for raw SVG export, `mermaid-result(...)` for structured render payloads, `validate-mermaid(...)` for validation-only workflows, and `error-mode: "panic" | "placeholder" | "text"` for draft-friendly error handling.
- Added `xtask build-typst-package`, which builds the Typst-compatible wasm and assembles `dist/typst/merman/<version>` with `typst.toml`, `lib.typ`, README, examples, licenses, and the wasm plugin.
- Started the Typst package on an independent `0.1.x` version track instead of locking it to Cargo prerelease versions, because Typst imports require numeric package versions.
- Added CI smoke coverage for the Typst package: package build, wasm ABI/size gate, example compilation, and `@preview` import smoke.
- Added Typst examples for basic usage, raw blocks, options, print-friendly output, slide-sized dark output, SVG export, and structured render results.

### Changed

- Consolidated `merman-core`'s public feature surface into coarse-grained profiles: `full`, `full-config`, `full-sanitization`, and `host`.
- Kept default builds Mermaid-compatible with `full + host`, while making `--no-default-features` a meaningful pure-WASM/Typst starting point.
- Split `merman-render`'s `core-full` forwarding from its host feature so Typst render builds can keep parser/layout/SVG support without pulling full config and sanitizer dependencies.
- Made render/layout timing and RoughJS seed-zero randomness deterministic in no-host wasm profiles, while preserving host behavior behind explicit host features.
- Collapsed the Typst wasm rendering surface to `render_svg_json` plus `validate_json`; the older direct `render_svg` export was removed before the Typst package was published so all Typst rendering uses one structured result path.

Feature guidance:

- Most Rust applications should keep defaults. That means `merman` still enables Mermaid-compatible full config/sanitization and host behavior.
- Use `default-features = false` when embedding the parser/core in a pure wasm environment that cannot import host time, random, URL, YAML, JSON5, or sanitizer dependencies.
- Enable `render` without `core-full` for Typst-like SVG rendering where the source and options are trusted or already normalized and package size matters.
- Enable `core-full` when you need Mermaid's broad config/frontmatter surface, YAML/JSON5 parsing, or full sanitizer parity.
- Enable `host` when the renderer should use local wall-clock behavior or host randomness. Leave it off for deterministic wasm output.

### WASM Footprint & Typst Compatibility

- Slimmed the pure/Typst-oriented core profile significantly. A Typst-compatible `wasm32-unknown-unknown` semantic probe built on `merman-core --no-default-features` measured **1,737,728 bytes raw** (**570,804 bytes gzip**), while the metadata probe measured **1,736,363 bytes raw** (**570,150 bytes gzip**).
- A core-only no-import probe measured **1,729,398 bytes raw** (**567,208 bytes gzip**).
- The Typst-oriented probe imports only Typst's two `wasm-minimal-protocol` host callbacks and no longer pulls `wasm-bindgen`, `js-sys`, `serde_yaml`, `json5`, `lol_html`, `url`, `uuid`, or `web-time` through the pure/no-default core path.
- The default minimal Typst package build (`render`, no `core-full`, no host`) now measures about **7.02 MB raw** and **1.93 MB gzip** and passes the Typst wasm ABI gate with only the two `wasm-minimal-protocol` imports.
- The opt-in full no-host Typst render build (`render + core-full`) measures **8,073,841 bytes raw** (**2,349,176 bytes gzip**) with the same Typst-only import surface.
- Added repeatable WASM size budgets for browser and Typst presets. `xtask wasm-size-matrix` now reports raw, stripped, gzip, and brotli bytes, and CI fails if preset budgets regress.
- Reduced the generated default `@mermanjs/web` `browser-full` package artifact by building with the workspace `wasm-size` profile through `wasm-pack --profile wasm-size`. The generated wasm dropped from **8,648,002 bytes raw** to **5,580,151 bytes raw**; the current compressed sizes are **2,135,543 bytes gzip** and **1,589,052 bytes brotli**.

### Fixed

- Corrected web package documentation to use the published `@mermanjs/web` npm package name.
- Avoided clipped Flowchart edge labels in Linux/Firefox browser previews. Thanks @aurabindo for reporting [#2](https://github.com/Latias94/merman/issues/2).
- Limited CSS override cleanup to `<style>` blocks and `style` attributes so ordinary SVG text and metadata containing `!important` stay intact.
- Scoped embedded icon IDs so repeated Flowchart and Architecture icons do not collide inside one SVG.
- Scoped Sankey generated IDs and Sequence debug markers for safer inline SVG embedding.

## [0.7.0] - 2026-06-09

Merman 0.7.0 is the first non-prerelease 0.7 line. It stabilizes the Mermaid 11.15-compatible headless rendering surface for broader editor, web, CLI, rustdoc, and native-binding use, while keeping parity and quality gates explicit.

### Breaking Changes

- Carries forward the 0.7 alpha API changes: detector construction uses `for_pinned_mermaid_baseline()`, known-type parser methods use `*_with_type*`, raster sizing uses the new target-aware `RasterOptions`, and theme metadata APIs use the supported-theme naming.

### Added

- Added Venn diagram parsing, layout, and SVG rendering as beta coverage with upstream-backed fixtures and targeted SVG gates.
- Added host theme profiles and built-in editor-oriented theme presets so embedders can adapt diagrams to dark and themed host surfaces without rewriting per-diagram SVG output.
- Added theme discovery through Rust, WASM, FFI, UniFFI, and platform binding surfaces.
- Added copyable host-theme and stylized-theme Rust examples, plus broader theme smoke coverage across diagram families.
- Added a corpus-driven benchmark harness that compares native `merman`, `mermaid-rs-renderer`, and upstream Mermaid JS v11.15.0 with separate performance, coverage, missing, skipped, and error reporting.

### Changed

- Deepened render request planning, family metadata, headless operations, xtask comparison/admission flow, and theme role ownership so release-facing APIs rely on fewer implementation-era seams.
- Expanded playground theme preset support, share-state handling, preview status, and web package documentation around the published `@mermanjs/web` package.
- Updated release workflow examples and release documentation for the final `0.7.0` tag.

### Fixed

- Improved host-theme readability for labels, fallback text, ER relationship labels, requirement strokes, GitGraph branch/tag labels, and `resvg`-safe SVG output.
- Fixed GitGraph label vertical centering under non-default host themes.
- Fixed release-facing web package documentation that still referenced the unpublished `@merman/web` name.

## [0.7.0-alpha.2] - 2026-06-08

This alpha prepares the native, web, and editor-preview surfaces for external testing. It focuses on safer host integrations, clearer package APIs, and a smaller set of release-ready examples.

### Breaking Changes

- Replaced the stale Mermaid 11.12 registry constructors with `for_pinned_mermaid_baseline()`. Detector callers can also choose `pinned_mermaid_baseline_full()` or `pinned_mermaid_baseline_tiny()`.
- Renamed known-type parser methods from `*_as*` to `*_with_type*`, for example `parse_diagram_as_sync` -> `parse_diagram_with_type_sync`.
- Renamed theme metadata APIs to `supportedThemes()`, `supported_themes()`, and `merman_supported_themes_json()` to match the supported diagram metadata API.

### Added

- Added fixed-time render options for stable date-sensitive diagrams such as Gantt charts.
- Added copyable Rust examples, including a custom host output environment example.

### Changed

- Raised the MSRV to Rust `1.95`.

### Fixed

- Improved editor-preview stability for host apps such as Zed, including readable SVG and `resvg`-safe output.
- Hardened parser, layout, Graphlib/Dagre, sanitizer, and SVG cleanup paths against malformed or deeply nested input.
- Fixed Python wheel packaging so published wheels include native platform libraries.
- Fixed Flowchart zero-spacing defaults and class text preservation in preview/raster output.

## [0.7.0-alpha.1] - 2026-06-05

Merman 0.7 alpha.1 updates the renderer to Mermaid 11.15 compatibility and opens the first public surfaces for ASCII output, rustdoc rendering, web/WASM usage, and native FFI experiments.

### Breaking Changes

- Updated the compatibility target to Mermaid `11.15.0`. Refresh semantic, layout, or SVG goldens if your integration keeps parity snapshots.
- PNG/JPG raster output now applies a safety budget by default. Configure `RasterOptions`, `RasterSizeLimit`, or unbounded raster output for very large diagrams.
- `RasterOptions` gained target-aware sizing fields. Exhaustive struct literals should add `..Default::default()` or set the new fields explicitly.

### Added

- Added ASCII/Unicode rendering through `merman-ascii`, `merman::ascii`, and `merman-cli render --format ascii|unicode`.
- Added `merman-rustdoc` for rendering Mermaid fences and `include_mmd!` files as inline rustdoc SVG without injecting Mermaid JavaScript.
- Added the `@mermanjs/web` TypeScript/WASM package and a hosted playground with live editing, SVG export, Mermaid compare mode, diagnostics, benchmarks, and examples.
- Added experimental native bindings for C ABI, Flutter/Dart, Android JNI, Apple SwiftPM, and Python UniFFI.
- Added initial support for more Mermaid 11.15 diagram families, including TreeView, Ishikawa, and Event Modeling.

### Changed

- Theme handling now follows Mermaid 11.15 more closely, including supported theme metadata, `look`, `themeVariables`, and scoped `themeCSS`.
- Raster export plans output size before allocating buffers, while SVG output remains parity-oriented.

### Fixed

- Fixed many Mermaid 11.15 rendering gaps across Flowchart, Sequence, Class, Architecture, State, Block, Timeline, Pie, Radar, Treemap, Mindmap, ER, Journey, Requirement, Sankey, C4, and XY Chart.
- Fixed dark-host and custom-theme visibility issues for labels, notes, edges, clusters, and chart elements.
- Fixed deeply nested valid diagrams that could hit stack-sensitive parser or layout paths.
- Fixed oversized raster exports and JPG background handling.

## [0.6.0] - 2026-05-28

This release adds an opt-in SVG output pipeline for applications that need Mermaid-parity SVG by default but also need cleaner output for in-app previews, PNG/PDF export, or host-specific theming. Use `render_svg_sync` for parity snapshots, `SvgPipeline::readable()` when the SVG will be inlined and should keep readable fallback text, and `SvgPipeline::resvg_safe()` before rasterizing through `resvg` / `usvg`.

### Added

- Added `SvgPipeline::readable()` and `SvgPipeline::resvg_safe()` for callers that need fallback text, rasterizer-friendly SVG, or cleanup without changing default `render_svg_sync` output.
- Added host styling extension points: `SvgPostprocessor` for custom passes, `ScopedCssPostprocessor` for CSS injection, and `CssOverridePolicy::StripExistingImportant` for callers that want app styles to override Mermaid defaults. Postprocessors can read the diagram type, title, and root SVG id from `SvgPostprocessContext`.
- Expanded Zed-derived regression coverage for Sequence, Flowchart, ER, Gantt, Class, and raster fallback cases.
- Added crate-specific README pages for `merman-core`, `merman-render`, and `merman-cli`, including focused parsing, rendering, and CLI examples for docs.rs/crates.io users.
- Added a rendering guide in `docs/rendering/SVG_OUTPUT_PIPELINE.md` and a runnable `svg_pipeline` example:

  ```bash
  cargo run -p merman --features render --example svg_pipeline < fixtures/flowchart/basic.mmd > out.svg
  ```

  Library integrations can use the same pipeline directly. This example builds a typical editor/export pipeline: make the SVG `resvg`-friendly, allow host CSS to override Mermaid defaults, and scope the injected CSS to one diagram id.

  ```rust
  use merman::render::{
      CssOverridePolicy, HeadlessRenderer, ScopedCssPostprocessor, SvgPipeline,
  };

  let pipeline = SvgPipeline::resvg_safe().with_postprocessor(
      ScopedCssPostprocessor::new(
          r#"
  .node rect {
    stroke: #2563eb;
    stroke-width: 2px;
  }
  .merman-foreignobject-fallback-text {
    fill: #111827;
  }
  "#,
      )
      .with_override_policy(CssOverridePolicy::StripExistingImportant),
  );
  let renderer = HeadlessRenderer::new().with_diagram_id("host-diagram");
  let svg = renderer
      .render_svg_with_pipeline_sync("flowchart TD; A[API]-->B[DB];", &pipeline)?
      .unwrap();
  # let _ = svg;
  # Ok::<(), Box<dyn std::error::Error>>(())
  ```

### Changed

- Readable SVG helpers, raster helpers, and CLI raster export now use the shared SVG output pipeline; default `render_svg_sync` remains Mermaid-parity output with no consumer cleanup.
- `ScopedCssPostprocessor` now inserts host CSS after existing SVG styles when possible, so scoped host rules follow Mermaid defaults in cascade order.

### Fixed

- Fixed Architecture arrowheads on diagonal edges so they follow the rendered line direction.
- Fixed readable/raster output for Mermaid HTML labels: fallback text now handles literal `\n`, avoids double-escaped entities such as class generics, and keeps useful styling context for host CSS.
- Fixed sequence diagrams with keyword-like participant ids such as `AS`, `END`, `RECT`, or `loop`.
- Hardened `SvgPipeline::resvg_safe()` against common `usvg` / `resvg` incompatibilities, including unsupported CSS, animation declarations, invalid visual attributes, empty rectangle placeholders, CSS `deg` units, and non-finite values.

## [0.5.0] - 2026-05-19

This release is mostly about rendering fidelity and the render pipeline. If you render diagrams to SVG, PNG, JPG, or PDF, the main difference is fewer label, sizing, and viewport mismatches against Mermaid 11.12.3. The public semantic JSON API stays available, while render-only paths now avoid more of the old JSON round trip.

### Added

- Sequence diagrams can now measure and render KaTeX/math labels in actors, messages, notes, boxes, and block labels when the Node KaTeX backend is available.
- Added release and parity tooling for maintainers: stricter SVG parity verification, root viewport audits, override growth checks, and root-delta reports across diagram families.
- Added benchmark and migration notes for the typed render-model work, including current performance baselines for render-heavy paths.

### Changed

- Render-only flows now use typed render models across more diagram families instead of repeatedly converting through semantic JSON. This covers Sequence, Kanban, Gantt, Pie, Packet, Timeline, Journey, Requirement, Sankey, Radar, Info, ZenUML, QuadrantChart, GitGraph, Treemap, Block, and ER.
- Flowchart, Sequence, GitGraph, State, Mindmap, Requirement, Journey, Timeline, ER, Architecture, and Class rendering now match Mermaid 11.12.3 more closely for HTML labels, SVG text, icons, titles, actor/message/note sizing, styled labels, and root viewports.
- Render config parsing is shared across layout and SVG rendering, including numeric strings and CSS `px` values.
- Class, Sequence, Architecture, and shared text rendering code were split into smaller modules. This should make future parity fixes easier without changing the public API.
- Hot render paths avoid several unnecessary clones and temporary allocations, especially in Sequence, Flowchart, Class, and typed render-model dispatch.

### Fixed

- Fixed Flowchart HTML label measurement for repeated short glyph runs, multi-hyphen labels, icon labels, custom FontAwesome fallbacks, subgraph titles, fork/join shapes, and numeric spacing config.
- Fixed GitGraph branch, commit, tag, title, theme-font, and seeded auto-id behavior so generated SVGs line up more closely with Mermaid's parse-before-render pipeline.
- Fixed Sequence title, actor, message, note, block, line-break, font-size, and math-label sizing cases that could produce incorrect output bounds.
- Fixed State, Mindmap, ER, Journey, Requirement, Timeline, and Architecture sizing edge cases that affected exported SVG viewport dimensions.
- Removed many production `unwrap` and `expect` paths from parser, layout, and render code and replaced them with explicit error handling or safer control flow.

### Removed

- Removed the obsolete `parse_diagram_for_render_sync` compatibility API and its async alias. Use `parse_diagram_for_render_model_sync` for render-optimized parsing, or `parse_diagram_sync` when you need semantic JSON.
- Removed old Mindmap and State JSON-for-render helper paths.
- Removed the stale `merman-render/flowchart_root_pack` experimental debug feature.
- Removed the generated Class root-viewport table after typed calibration covered those cases.

## [0.4.0] - 2026-03-12

### Added

- `xtask`: support custom fixture roots in SVG baseline generation/comparison, add Markdown-aware text measurement, and integrate an opt-in Node/Puppeteer KaTeX path when `tools/mermaid-cli` is available.
- Docs: add and expand `docs/workstreams/*` parity planning material, including root viewport (`parity-root`) checks and text-measurement alignment notes.
- Tests/Fixtures: add a broad parity corpus covering font-size precedence, HTML label wrapping, Markdown `<br/>` continuations, unknown XML entities, KaTeX flowcharts, text-style overrides, and root viewport probes across multiple diagram types.

### Changed

- Text parity work now consolidates large amounts of fixture-derived width/height/padding data into generated `*_text_overrides_11_12_2` tables instead of leaving diagram-specific literal branches inline across layout/render code.
- SVG/style precedence now follows Mermaid more consistently: `themeVariables.fontSize` and `themeVariables.fontFamily` win where upstream uses them, and parity tooling captures more text-style drift during SVG comparison.

### Fixed

- Text/Markdown: align shared HTML/SVG text handling with Mermaid for inline code, failed `__` delimiter runs, paragraph-vs-raw-block HTML labels, punctuation-heavy URL wrapping, hyphenated-token min-content width, and trailing whitespace height edge cases.
- Flowchart: align HTML/SVG label wrapping, class/style text application, entity decoding, edge-label DOM/background/root bbox behavior, and complete the upstream Cypress new-shapes strict-XML buckets.
- Class: reduce strict-XML drift across note labels, namespaces, generics, relations/cardinality terminals, style propagation, annotation-driven sizing, and SVG/HTML title/member width measurement.
- ER: align relationship-label Markdown/backtick handling, root `htmlLabels` semantics, and entity/root font-size precedence with Mermaid baselines.
- State/Class/Mindmap/Kanban/Architecture: align remaining HTML label widths, wrapping-width handling, shared text constants, width parsing, and icon/service label fallback geometry between layout and SVG render.
- Block: complete strict XML parity for the Mermaid block corpus and align remaining marker-aware terminals, `space:N` handling, HTML label sizing, and shape-specific geometry.
- Requirement/GitGraph/Timeline/Treemap/Sequence/Sankey/C4/Journey/Pie/Radar/XYChart/Gantt: move repeated text constants into generated overrides and close the remaining text-geometry, viewport, and font-size precedence gaps that affected parity fixtures.
- Theme/CSS: stop implicitly applying `base` defaults under `theme=default`, seed Mermaid-like base/neutral xychart defaults, and prefer `themeVariables.fontFamily` in emitted root SVG styles.
- Core/Layout internals: clean the remaining strict Clippy offenders in `dugong-graphlib`, `dugong`, and parser helper code, and scope vendored `manatee` FCoSE lint exceptions to the algorithm module so current stable Clippy stays actionable outside the imported numeric code.
- Toolchain/CI: pin the workspace Rust toolchain to `1.87.0` and make CI install the same version explicitly, so release and local checks stop drifting with floating `stable`.
- Toolchain/CI: drop GitHub Actions `cargo fmt` / `cargo clippy` steps for now so release CI focuses on build, tests, and parity checks while the remaining render hot spots are still being aligned.
- Maintenance: normalize `rustfmt` output in parity/text/timeline/xtask helpers so the pinned toolchain now passes workspace format checks without local-vs-CI drift.

## [0.3.0] - 2026-03-02

### Added

- Promoted additional in-scope deferred fixtures into the committed corpus (state parser specs, flowchart icon specs, class diagram specs, and math examples) and generated upstream SVG baselines.

### Fixed

- Architecture: refresh compound bounds after FCoSE spring iterations before applying `relocateComponent`-style centering (fixes `parity-root` root `max-width` drift in deep compound/group fixtures).
- Flowchart: unescape quoted string labels (e.g. Windows paths like `C:\\Temp\\...`) and preserve Unicode punctuation in label text.
- `xtask compare-flowchart-svgs`: skip ELK flowchart fixtures requested via `layout: elk` / `flowchart.defaultRenderer=elk` (prevents layout failures while ELK parity is deferred).
- Flowchart: align icon node shape rendering with upstream Mermaid (`icon` vs `iconSquare`) to avoid NaN path data and restore SVG DOM parity for AWS icon fixtures.
- Flowchart: improved `iconSquare` RoughJS path parity (rounded-rect path structure) for upstream icon shape fixtures.
- Class: align `htmlLabels` split semantics more closely with Mermaid: notes now respect global `htmlLabels` + class padding, while relation title labels switch to SVG `<text>/<tspan>` + background groups only when `flowchart.htmlLabels=false` is explicitly active.
- Class: render `htmlLabels: false` labels via SVG `<text>/<tspan>` (avoid `<foreignObject>` DOM mismatches in parity baselines).
- Text: closer-to-upstream Mermaid Markdown tokenization for flowchart SVG labels and layout measurement (fixes underscore/emphasis boundary edge cases).
- Radar: fixed detailed-entry parsing so decimal values like `3.2` are not misparsed as axis `3` with value `0.2`.
- Treemap: tightened header parsing to match Mermaid CLI (`treemap:` / `treemap utilities` now fail) and preserved the upstream behavior where trailing whitespace-only lines are treated as a syntax error.
- `xtask audit-gaps`: avoid trimming trailing whitespace when parsing deferred fixtures (prevents false “parse OK” on grammars like Treemap that treat trailing whitespace-only lines as an error).
- `xtask audit-gaps`: added `--check-upstream-render-deferred-ok` to identify promotable deferred fixtures (in-scope + upstream render OK).
- `xtask` SVG DOM compares: further reduced noisy `parity-root` root viewport diffs by snapping `max-width`/`viewBox` to a coarser lattice (0.25px).
- `xtask gen-upstream-svgs` / `compare-state-svgs`: allow generating/validating upstream baselines for renderable state parser fixtures while skipping the known upstream-crashing `upstream_state_parser_spec` fixture.
- Architecture: improved compound/nesting layout alignment by extending the FCoSE port with a compound graph model and closer-to-upstream bounds/centroid propagation behavior.
- Architecture: improved edge parsing/modeling compatibility (including `lhsInto`/`rhsInto` metadata when present).
- Architecture: removed fixture-id keyed label wrapping/formatting special-cases by tightening `createText(...)`-like SVG label wrapping and matching Mermaid CLI attribute newline serialization (`&#10;`).
- `xtask` SVG DOM compares: stabilized anonymous edge wrapper ordering for Architecture and reduced non-actionable text diffs caused by line wrapping sensitivity.
- README: fixed the Stress gallery Architecture fixture reference and refreshed the Architecture showcase render.

### Not Released / WIP

- Architecture: geometry-level parity (placements, viewport, and routing coordinates) is still being aligned to upstream Cytoscape/FCoSE. SVG DOM parity is compared in `dom-mode parity`, so expect occasional layout snapshot churn while we tighten numeric fidelity.
- Flowchart: HTML-label `$$...$$` (KaTeX) fixtures now participate in strict DOM parity via the opt-in `NodeKatexMathRenderer`; only environments without the local `tools/mermaid-cli` toolchain still fall back to non-math comparisons.
- Flowchart: `flowchart-elk` layout is not implemented yet; compare tooling skips those fixtures (still kept in the corpus for parser coverage).
- `merman-core`: dropped support for legacy Architecture edge shorthand (e.g. `a L--R b`, `a (L--R) b`) to align with Mermaid@11.12.3's Langium parser; use port-colon syntax instead (e.g. `a:L -- R:b`).
- `merman-render`: introduced a pluggable `MathRenderer` interface for `$$...$$` math labels (no default KaTeX backend; pure-Rust remains the default).
- `xtask`: added `audit-gaps` to summarize parser-only fixtures and deferred corpus status (helps drive “missing implementation” work off reproducible reports).
- `xtask audit-gaps`: optionally probe upstream renderability for parser-only fixtures via Mermaid CLI (flags: `--check-upstream-render`, `--upstream-timeout-secs`).

## [0.2.0] - 2026-02-26

### Added

- Imported additional upstream fixtures from Cypress and package tests (requirement, gantt, ER, flowchart, sequence, state, class, quadrantchart, xychart, radar, kanban, architecture, block, mindmap, timeline) to expand SVG parity coverage.
- Imported additional upstream fixtures from Mermaid's parser package tests (architecture, gitgraph, info, packet, pie) to expand SVG parity coverage.
- Imported upstream HTML demo fixtures (flowchart, sequence, quadrantchart, sankey, xychart) to expand golden-driven parity coverage.

### Fixed

- Improved `<foreignObject>` readability fallback for raster outputs (PNG/JPG/PDF): remove the white text outline overlay and render a semi-transparent `.labelBkg` background when present (closer to upstream Mermaid defaults).
- Reduced cross-platform SVG DOM drift in `parity-root` compares by snapping root `style` `max-width` and `viewBox` to a stable lattice.
- Further reduced `parity-root` drift by bias-snapping root `max-width` and masking `viewBox` origin (x/y) while still tracking viewport size changes (w/h).
- Block: aligned `doublecircle` SVG structure to match upstream Mermaid DOM output.
- Aligned C4 `sprite` rendering with upstream Mermaid: only `person`/`external_person` emit `<image>` sprites.
- ER: align Markdown formatting in entity labels even when the entity has no attributes.
- Flowchart: preserve cyclic self-loop helper mid-edge labels (fixes missing self-loop label DOM).
- Pie: support `accTitle:` / `accDescr:` on the header line (as accepted by upstream Mermaid parser tests).
- `import-upstream-pkg-tests`: avoid failing the import when all candidates are skipped (still prints a skip summary).
- `import-upstream-pkg-tests --with-baselines`: defer fixtures that fail upstream baseline generation / render as upstream error output under `fixtures/_deferred/` (keeps the corpus without breaking parity gates).
- Reduced churn during `import-upstream-docs --with-baselines` by skipping blank-info code fences that lack an explicit Mermaid diagram directive (e.g. `flowchart` / `graph`).
- Reduced churn during `import-upstream-cypress --with-baselines` by deferring out-of-scope class fixtures (`htmlLabels=false`, `layout=elk`, `look!=classic`) under `fixtures/_deferred/`.
- Improved `import-upstream-pkg-tests` Mermaid source extraction to handle `"..."` / `'...'` literals and template strings with `${...}` interpolation.
- Sequence: render diagram titles from metadata/frontmatter when the semantic model title is empty (aligns upstream HTML demos).
- Sequence: adjusted wrapped note line breaks to match upstream Mermaid `wrapLabel(...)` behavior (11.12.3 baselines).
- QuadrantChart: derive default theme colors from `themeVariables` (including `hsl(...)`/hex parsing) to match upstream theme behavior.

### Changed

- Refreshed README showcase renders after parity updates (architecture/mindmap/sankey/gantt).
- CI: run `parity-root` SVG DOM comparisons as a non-blocking check on Ubuntu (keeps `parity` as the gate).
- Documented that the root viewport override baselines track Mermaid 11.12.3 (override module filenames still use the historical `*_11_12_2.rs` suffix).
- Updated upstream Mermaid baselines to 11.12.3 and refreshed `fixtures/upstream-svgs/**`.
- `import-upstream-html`: flowchart fixtures containing `$$...$$` math labels now use the stable `*_katex` suffix and participate in full SVG DOM parity when the local KaTeX backend is available.
- Deferred upstream HTML treemap demos that render as upstream error output under `fixtures/_deferred/` (avoid permanently failing parity gates).

### Removed

- Removed `mermaid-rs-renderer` (`mmdr_`) fixtures and baselines from this repository; fixtures are now sourced only from upstream Mermaid.

## [0.1.0] - 2026-02-22

### Added

- Headless Mermaid parsing and semantic JSON output (`merman-core`).
- Headless layout + SVG rendering with DOM parity gates against upstream baselines (`merman-render`).
- Ergonomic wrapper crate for UI integrations (`merman`, feature-gated via `render` / `raster`).
- CLI for detection, parsing, layout, and rendering (`merman-cli`).
- Raster outputs (PNG/JPG/PDF) via pure-Rust SVG conversion (`resvg` / `svg2pdf`).
- Golden snapshots and parity tooling (`xtask`, `fixtures/**`, `docs/alignment/STATUS.md`).
- ZenUML headless compatibility mode (subset translated to `sequenceDiagram`; not parity-gated).
- Local performance regression tracking via Criterion (`cargo bench -p merman --features render --bench pipeline`).

### Changed

- SVG renderer implementation is organized under `svg::parity` to reflect the upstream-as-spec intent.
- State diagram root viewport (`viewBox`/`max-width`) defaults to SVG-emitted bounds scanning (closest to browser `getBBox()`); set `MERMAN_STATE_VIEWPORT=layout` to use layout-derived bounds.
