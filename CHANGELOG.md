# Changelog

All notable changes to this project will be documented in this file.

The format is based on *Keep a Changelog*, and this project adheres to *Semantic Versioning*.

## [Unreleased]

### Breaking changes

- Updated the compatibility target from Mermaid `11.15.0` to `11.16.0`; integrations that retain semantic, layout, or SVG parity snapshots should regenerate them.
- Replaced the TextScan-capable `AnalysisFactsPayload` shape shipped in `0.8.0-alpha.3` with the sole parser-only facts v1 contract. `fact_source: "text_scan"` is removed, unavailable bodies use `"unavailable"`, and semantic items require `rename_policy`. This deliberately resets the alpha wire contract before stable release; no legacy decoder or dual facts path is retained. The diagnostics-only `AnalysisPayload` independently remains v1. Facts schema versions are independent of native, UniFFI, and WASM ABI versions, LSP document revisions, and Mermaid `*-v2` ids.
- Closed the alpha Rust editor parser API: `Engine::parse_diagram_with_editor_facts_sync` is replaced by `parse_diagram_snapshot_sync` / `parse_diagram_snapshot_with_type_sync`; `parse_metadata{,_sync}` no longer accepts `ParseOptions` or returns `Option`, and `Engine::parse`, `ParseOptions::suppress_errors`, and the VS Code `merman.analysis.parse.suppress_errors` setting are removed. Use model-producing parse or render APIs when an integration previously suppressed parse errors.
- Kept the prerelease C, Android JNI, Apple Swift, Flutter/Dart, UniFFI, and WASM ABI identifier at version 2 while replacing the alpha host text-measurement callback contract in place. Requests now identify one of 19 exact operations in addition to the routing phase, and handled callbacks must return the operation's explicit tagged result kind. Operations 15 and 16 expose Mermaid's calculated text dimensions and Canvas width; operation 17 distinguishes Architecture's inherited middle-baseline bbox y from operation 14's ordinary createText bbox y; and operation 18 exposes raw SVG `<text>.getBBox().height`. Rebuild native artifacts, regenerate bindings where applicable, and update callbacks for all operation codes 0 through 18.
- Expanded the alpha ABI 2 diagram-family capability record in place. C JSON, WASM, UniFFI/Python, Swift, and Flutter now expose logical and render-model identities, detector, semantic/editor/combined parser, typed-render and authoring-header flags, and the config namespace from the sole Rust family catalog. Regenerate bindings and update hosts that decode the previous four-field UniFFI record.
- Renamed the public Rust accessor `merman_analysis::FenceDelimiter::len()` to `marker_len()` without a deprecated alias; update callers to use the replacement name.
- Renamed the public Rust type `merman_core::diagrams::flowchart::FlowchartV2Model` to `FlowchartModel` without retaining a deprecated alias. Mermaid's `flowchart-v2` diagram id and the compatibility layout JSON `FlowchartV2` variant key are unchanged.
- Removed the public low-level `merman-render` `layout_parsed*`, `render_layouted_svg`, raw semantic/layout SVG helpers, debug wrappers, and per-family pass-through render functions. Use `merman::render::HeadlessRenderer`, `prepare_render_sync`, `layout_json_sync`, or `render_svg_sync`; direct low-level callers can use `merman_render::family::prepare` with one `RenderSession`.
- Moved production text measurement, math and icon services, clock, randomness, and resource limits into `RenderEnvironment`. Layout and SVG options no longer select independent services; binding and Web JSON now use `host_theme.theme_variables`, `environment.text_measurement`, and `environment.math_renderer`, while legacy `host_theme.themeVariables`, `layout.text_measurer`, and `layout.math_renderer` paths are rejected; `SvgRenderOptions` carries request values and `SvgDebugOptions` carries diagnostics.
- Added effective `font_style` to `merman_render::text::TextStyle` and removed the redundant `measure_wrapped_raw` / `WrappedRaw` extension point. Custom measurers should implement `measure_wrapped` from the complete style; successful host measurements are no longer followed by vendored whole-label style adjustments. The public heuristic-only `wrap_text_lines_px` helper is also removed; callers that need wrapping should use `wrap_text_lines_measurer` with their selected `TextMeasurer`.
- Removed `RootViewportOverridePolicy`, generated root pins, exact-label text/SVG tables, and their audit/generator commands. Root viewports are always computed from source-backed family or emitted-content bounds; successful host font measurement bypasses vendored fallback facts.
- Renamed `LayoutOptions.viewport_width` / `viewport_height` and the corresponding binding, Web, and Typst request fields to `container_width` / `container_height` (Typst: `container-width` / `container-height`) without aliases. These values describe the host layout container, not a browser page viewport; callers using the removed names now receive an options error. The CLI keeps `--width` / `--height` and removes the misleading `--viewport-width` / `--viewport-height` aliases.
- Removed `LayoutOptions::use_manatee_layout` without an alias. The `cytoscape-layout` feature is now the sole capability boundary: enabled builds always execute Architecture FCoSE and non-`tidy-tree` Mindmap COSE-Bilkent layout, while disabled builds report those families as unsupported.
- Removed the alpha `FlowchartElkBackend` selector and its CLI, binding JSON, and Web options without aliases. Flowchart ELK now always uses the source-backed Mermaid adapter and Eclipse ELK layered implementation; the older lightweight compatibility backend is removed.
- Replaced the browser `createBrowserTextMeasurer()` helper with the owned `createBrowserTextMeasurementSession()` contract. Browser hosts must retain the returned `measure` callback for the session and call `dispose()` when its realm/session ends; no deprecated alias is retained.
- Renamed web `editorSemanticTokenLegend()` to `editorSemanticTokenDescriptor()` and changed `editorSemanticTokens()` from token objects to a packed `Uint32Array`. Consumers must load the generated descriptor before decoding tokens; no compatibility API remains.
- Changed the public ZenUML typed model so statement endpoints expose explicit and resolved options, statement numbering is renderer-owned, group ids are optional, and participant widths retain their source lexeme. Callers that inspect `ZenumlDiagramRenderModel` must update their field access.
- Changed the Typst transport resource contract to enforce the `typst-package` policy for every call. Caller-provided `resources` profiles and numeric overrides are replaced at the plugin boundary instead of being allowed to select trusted or unbounded limits.
- Removed underscore and shorthand aliases for binding enum values. Use the documented kebab-case forms such as `resvg-safe`, `strip-existing-important`, `typst-package`, `trusted-native`, `unbounded-for-trusted-input`, and the generated host-theme preset names; the Rust parser and Web types now expose the same closed value set.

### New and changed

- Separated oversized-output policy by format. SVG remains uncapped vector markup; PNG/JPG now preflight fit, scale, final pixmap dimensions, and embedded images through `RasterOptions`; and vector PDF uses independent `PdfOptions` page, filter-bitmap, and embedded-image budgets. Markdown batches additionally bound aggregate encoding memory. The CLI adds raster fit/limit controls, PDF filter controls, `--embedded-image-max-pixels`, `--embedded-image-max-total-pixels`, `--embedded-images-unbounded`, and `--encoding-memory-budget-mib`, reports effective constrained dimensions, and maps `--pdfFit` from 96 CSS pixels to 72 PDF points.
- Rebuilt the Playground around one document-owned Merman lifecycle and a separate latest-wins Render Coordinator. WASM import/fetch run concurrently, browser HTTP cache is the only persistent byte-cache authority, retry is staged and bounded, BFCache suspend/resume is distinct from final disposal, and the product renders the actual source without a hidden synthetic warmup.
- Added `@mermanjs/web/editor` backed by the `browser-editor` preset: the full 35-family catalog, analysis, and `merman-editor-core` language intelligence without SVG, ASCII, host, or ELK dependencies. The Playground runs it in a dedicated local module Worker and projects diagnostics, completion, hover, code actions, symbols, navigation, rename, and semantic tokens into Monaco while keeping native ABI 2 and editor/analysis/facts schema 1.
- Replaced the handwritten Playground example registry with an exact 35-family fixture manifest and generated typed catalog. Search, provenance, canonical detection, and generated-output freshness are checked by `xtask` against the Mermaid 11.16 baseline.
- Isolated Compare and Bench in authenticated same-origin realms. Compare owns one failure-resilient Mermaid operation queue; Bench uses equivalent per-engine realms, versioned raw phase events, equal real-source warmups, deterministic balanced AB/BA order, explicit failure/invalidation states, fail-closed ratios, and local JSON evidence download.
- Added parser and editor facts, typed layout, and SVG rendering for `cynefin-beta` and all four Railroad dialects: `railroad-beta`, `railroad-ebnf-beta`, `railroad-abnf-beta`, and `railroad-peg-beta`. ABNF repetition bounds and the public `RailroadRepeatBound` now preserve Mermaid's JavaScript number/binary64/infinity semantics. #21 #24
- Added source-backed `swimlane-beta` parser and editor facts through shared Flowchart semantics, plus dedicated typed Swimlane layout, routing, and SVG rendering; Swimlane now participates in the primary Mermaid 11.16 parity matrix.
- Added source-backed Wardley semantics, parser lexemes, typed layout, theme behavior, SVG, and root-viewport evidence to the 35-family primary matrix. ZenUML grammar and native SVG behavior now align with the admitted ZenUML Core 3.50.1 candidate while retaining the Mermaid 11.16 dependency graph as an explicit oracle.
- Aligned Mermaid 11.16 behavior across existing diagrams, including Flowchart and State self-loops, Sequence blocks and wrapping, Ishikawa recursive DOM structure, TreeView ordering, XYChart point labels, Architecture hints, Pie highlighting, Gantt timing, and config/frontmatter handling.
- Consolidated built-in ids, aliases, profile gates, semantic/editor/render adapters, metadata, configuration namespaces, and authoring headers in one Diagram Family catalog. Built-in compatibility JSON, parser-backed editor facts, and typed render models now project family-owned semantic construction instead of maintaining successful parallel parsers.
- Made the canonical headless operation typed from family semantics through layout and SVG. `FamilyRenderArtifact` owns an opaque matching semantic/layout pair, compatibility layout JSON projects from that artifact, and custom JSON parser models report an explicit non-renderable capability.
- Routed parity commands through the same typed `HeadlessRenderer` operation used by public callers and report the resolved render path and environment policy; compatibility JSON checks remain explicit projection tests rather than the SVG oracle.
- Centralized every built-in root SVG under Root Viewport policy, including fixed/responsive sizing, accessibility chrome, escaping, attribute order, and deferred finalization from computed late-bound content. Fixture-scoped pins are removed, and browser-only root differences remain explicit verification residuals rather than production data. #22
- Upstream SVG tooling now verifies pinned source, renderer runtime, browser timezone and fonts, input, and SVG provenance; promotes complete family batches transactionally under cross-process locks; removes obsolete baselines atomically; prevents compare readers from racing shared Mermaid CLI installs; and captures root attributes from the locked generation.
- Parity gates now compare the complete mismatch set against narrow, family-local evidence catalogs, so changed or additional mismatches still fail. Sequence currently has no accepted DOM residual.
- All 35 full-profile families now emit parser-owned lexemes through one validated, non-overlapping token planner shared by editor-core, LSP, WASM, Monaco, and the artifact-only VS Code extension.
- Added strict WASM input-closure manifests for every browser surface and a pinned Mermaid companion reference graph covering package locks, source commits, published tarballs, and ZenUML candidate admission.
- Added libFuzzer harnesses for parser, renderer, SVG pipeline, and C ABI boundaries, with sanitizer CI and fail-closed fuzz configuration checks. #25
- Added a machine-readable third-party component contract, exact upstream legal texts, deterministic Cargo/npm dependency reports, per-artifact legal projections, Cargo license policy, and package-content gates for all 20 publishable crates and native/Web release surfaces.
- Hardened release gates to verify artifact provenance, exact uploaded assets, package contents, and scoped workflow credentials before publication.
- Made Android source builds reproducible through the checked-in Gradle Wrapper and one version catalog for Java 17, AGP 9.2, NDK r29, and Dokka 2.2. The build helper can discover an installed JDK 17, install the pinned NDK, assemble both published ABIs, and verify the completed AAR in one command; the local Maven gate validates POM metadata, checksums, native slices, sources, and generated API documentation.

### Fixes and polish

- Simplified and polished the Playground shell with persistent editor/config/preview state, generated example search, keyboard-correct tabs and dialogs, synchronized system theme, safe-area/dynamic-viewport sizing, local Monaco assets, and accessible labels/focus behavior.
- TreeView now embeds configured Iconify pack bodies at Mermaid's 14px size and shows the standard unknown icon for missing packs or entries. #23
- Kept centered Railroad choice branches straight when equivalent lane coordinates differ only because of floating-point addition order. #22
- Fixed Mermaid 11.16 edge cases in TreeView annotation and highlight bounds; Cynefin inline syntax, frontmatter titles, and global fonts; Architecture reserved IDs; and generated XYChart axis defaults. #21
- Fixed multiline Packet `accDescr { ... }` parsing and aligned shared Langium title/accessibility spans across the families that import Mermaid's `common.langium` grammar.
- Applied Mermaid's staged theme calculation consistently across Radar, Kanban, Mindmap, and Timeline, including font-only and partial overrides; QuadrantChart now preserves raw browser inheritance while emitting a valid resvg-safe fallback.
- Made Block connectors terminate on shared visible shape geometry rather than nominal node boxes.
- Enforced Flowchart node, edge, subgraph, and label limits before Dagre, ELK, or Swimlane dispatch so a layout selector cannot bypass the configured resource policy.
- Preserved target-date daylight-saving semantics for Gantt parsing and layout instead of snapshotting the process's current UTC offset; boundary dates with fixed offsets now return `InvalidArgument` rather than overflowing in the CLI or safe binding APIs.
- Recorded deferred root SVG attribute ranges while emitting the root element, preventing valid diagram ids from colliding with internal `viewBox` or `max-width` placeholders.
- Unified fixture importer reject/defer rollback handling so all import sources restore the same transaction state after a failed baseline or deferred-fixture operation. #21

## [0.8.0-alpha.3] - 2026-07-09

0.8.0-alpha.3 turns Merman into a local Mermaid authoring tool, not only a renderer. You can lint from the CLI, talk to editors through LSP, call analysis APIs from browsers, and try the whole path in the new VS Code extension. ASCII output also covers more diagrams for terminals, docs, and text-only previews.

### Highlights

- You can now lint and edit Mermaid locally. The new analysis and LSP stack covers diagnostics, completions, hover, symbols, references, rename, folding, semantic tokens, and quick fixes. #20
- The new VS Code extension includes preview, diagnostics, source actions, snippets, SVG/PNG export, copy actions, and bundled `merman-lsp` / `merman-cli` binaries per platform. #20
- ASCII output is much more useful: state diagrams, sequence boxes and notes, class/ER relations, relation summaries, XYChart legends, extra Flowchart shapes, capability grades, and tighter dense layout fallbacks. #13 #17
- Web users can pick smaller package surfaces. `@mermanjs/web` now has capability-specific subpaths and metadata for render, ASCII, ELK, analysis, and editor-language builds. #20

### New crates

- `merman-analysis` is the render-free analysis layer. Use it when you want diagnostics, lint metadata, Markdown/MDX fence handling, or source ranges without pulling in SVG rendering. #20
- `merman-editor-core` contains editor behavior that is not tied to any protocol: completions, hover data, symbols, rename/navigation helpers, and semantic token inputs. #20
- `merman-lsp` packages the editor stack as a language server for VS Code and other LSP clients. #20

### Breaking changes

- `merman-cli` only loads icon packs from the network with `--allow-network`. Local `node_modules`, local JSON files, and `file://` icon packs still work by default. #19
- `merman-cli` keeps stdout for requested output bytes. Progress and non-error diagnostics go to stderr. #19
- `merman-cli` now uses exit code 2 for invalid input/config/output, 3 for direct I/O failures, and 1 for render/runtime failures. Broken stdout pipes exit successfully. #19
- `merman-core::Error::DiagramParse` now stores `diagnostic: ParseDiagnostic` instead of a top-level `message`. Rust callers should use `diagnostic.message()` for display text and `diagnostic.span()` / `diagnostic.span_kind()` when they need source locations. #20
- Slim `@mermanjs/web` subpaths now leave out unsupported runtime wrappers instead of exporting stubs that throw. Use the default entry point or `@mermanjs/web/full` when one import needs render, ASCII, and editor-language APIs together. #20
- The optional `merman` `egui-example` feature and desktop GUI example are gone. #19

### New and changed

- VS Code preview, diagnostics, source actions, and language intelligence can be turned on independently, so it is easier to run Merman next to other Mermaid tools. #20
- Python UniFFI ABI 2 adds reusable engines, diagram-family capability discovery, and host text-measurement callbacks. #20
- `merman-cli --svg-pipeline parity|readable|resvg-safe` lets CLI users ask for export-safe SVG bytes directly. #19

### Fixes and polish

- VS Code preview keeps the right source more reliably when Markdown files have multiple Mermaid fences, previews are locked, renders go stale, or users copy/export output. #20
- LSP diagnostics and semantic tokens no longer publish stale results after newer document edits. #20
- ASCII routing and fallback output are cleaner for nested subgraphs, relation labels, wide terminal cells, tight grids, and playground examples. #13 #15 #16 #17
- `resvg-safe` SVG output can remove duplicate native/fallback labels after raster-safe fallback generation. #19
- The `quick-xml` RustSec audit failure is gone after removing the stale GUI dependency path. #11

## [0.8.0-alpha.2] - 2026-06-23

This alpha focuses on Mermaid 11.15 parity, safer host integrations, and packaging readiness across CLI, Web/WASM, Typst, and native bindings.

### Breaking Changes

- The C ABI is now version 2 so native hosts can provide text-measurement callbacks; C, Android JNI, Apple Swift, and Flutter/Dart integrations should rebuild against the updated headers and wrapper APIs.
- Diagram-level CSS-sensitive config is filtered more strictly by default; move trusted `themeCSS`, font-family, and related theme overrides into trusted site config or the documented compatibility path when you intentionally need those overrides.

### Added

- Added source-backed ELK layout support for Flowchart and Class diagrams, including Mermaid-reachable ELK options such as `mergeEdges` and `nodePlacementStrategy`.
- Added `look: handDrawn` rendering support for Flowchart and Class diagrams, including deterministic seeding and fixes for rough edges, clusters, and decision shapes.
- Added browser/WASM host text-measurement APIs for `@mermanjs/web`, including `renderSvgWithTextMeasurer`, `layoutJsonWithTextMeasurer`, and `createBrowserTextMeasurer`.
- Added host text-measurement callbacks to the native binding surfaces for C, Android JNI, Apple Swift, and Flutter/Dart.
- Added Typst package improvements for document-context-aware diagrams, figure/layout controls, hardened document APIs, typography handling, and Typst 0.15 smoke coverage.
- Added `merman-cli completion <shell>` for shell completion generation while keeping `merman-cli` as the installed command name rather than adding an `mmdc` alias.
- Added repeatable WASM size reporting and budgets for browser and Typst presets, plus an opt-in `cytoscape-layout` feature for size-sensitive Architecture and Mindmap builds.
- Added Homebrew install guidance for `merman-cli` on macOS and Linux. Thanks @colindean for the contribution in #4.

### Changed

- Improved Flowchart ELK parity across compound subgraphs, cross-hierarchy edges, external ports, labels, self-loops, and source-backed layout defaults.
- Improved CLI ergonomics and functional `mmdc` workflow coverage by grouping help, separating top-level export flags from developer `render` options, and documenting command-name compatibility boundaries.
- Improved the playground and Web/WASM package so browser text measurement and Mermaid ELK layouts load on demand instead of forcing heavier default startup paths.
- Unified SVG-to-raster export through the same renderer-owned operation pipeline used by library and CLI callers, so sanitization, sizing, and encoding follow one path.
- Improved performance for layered layout and hot render paths, including Architecture and XYChart output, without changing public rendering APIs.
- Improved benchmark and parity tooling so release checks can cover source-backed ELK, hand-drawn output, WASM sizes, and Mermaid JS comparisons more repeatably.

### Fixed

- Fixed Journey raster rendering in the resvg pipeline. Thanks @vlasky for the contribution in #6.
- Fixed Flowchart hand-drawn decision-node silhouettes and other rough node shapes so `layout: elk`, `look: handDrawn`, and dark themes render more consistently.
- Fixed sanitized Flowchart click links, fallback text entity decoding, and several browser/host text-measurement edge cases that could affect exported SVG readability.
- Fixed Gantt `excludes` handling so diagrams that exclude every weekday, or otherwise produce a long run of excluded dates, fail with a parse error instead of looping during task date adjustment.
- Fixed WASM size-budget builds after dependency updates and made the size tooling respect `CARGO_TARGET_DIR` when locating build artifacts.
- Refreshed Kanban and Timeline layout snapshots to restore CI.

### Security

- Hardened `resvg_safe` SVG cleanup for raw SVG and rendered icon fragments by stripping active SVG elements, event-handler attributes, unsafe URL attributes, and unsafe style/presentation `url(...)` values while preserving local paint/reference URLs and raster data images.
- Hardened diagram config and Mermaid style parsing against CSS injection while preserving trusted site-level compatibility options.
- Added raster security regressions for default PNG/JPG pixmap budgets, custom raster size limits, and oversized intrinsic SVG rejection before PDF conversion.
- Added a `Security Audit` GitHub Actions workflow for Rust dependency changes and weekly scheduled audit runs.

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
- Avoided clipped Flowchart edge labels in Linux/Firefox browser previews. Thanks @aurabindo for reporting #2.
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
