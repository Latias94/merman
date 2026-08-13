# Changelog

All notable changes to this project will be documented in this file.

The format is based on *Keep a Changelog*, and this project adheres to *Semantic Versioning*.

## [Unreleased]

The next workspace release remains in development. This section records only completed user-visible outcomes since alpha.5; its final version and release scope have not been selected.

### Breaking changes

- Advanced the direct Apple/Python UniFFI binding API to `4` because lint rule catalog records now include required `tags`. API 4 replaces the generated `binding_api_version` probe with `transport_api_version` and removes the old native method symbol, so API 3 generated bindings reject the new library before decoding the changed record; regenerate and deploy each language projection with its matching native artifact.
- Default Android, Apple, Python, and Flutter native artifacts now bundle SVG, Cytoscape and ELK layouts, ASCII, analysis, validation, and document analysis, while omitting math, PNG, JPEG, PDF, and native clock/time-zone/random adapters. Generated wrapper methods remain stable and report typed missing-capability or unsupported-operation errors; consumers that need an omitted operation must build a current-contract custom native library.
- `DiagramParseOutcome::Parsed(Value)` is now `DiagramParseOutcome::Parsed { model, warning_facts }`. Rust editor integrations should match the struct variant and consume the parser-owned typed warning facts instead of decoding the compatibility model's `warningFacts` field.
- Broke the prerelease analysis facts wire contract to schema `2`: the generic parser/editor facts remain available, but the Flowchart-only rich graph field and its Rust/TypeScript/FFI projections are removed. Semantic roles are now the explicit `entity`, `class_definition`, `reference`, `outline`, and `payload` set, so exhaustive consumers must handle the two added declaration/reference roles. Facts consumers must regenerate against schema `2`; diagnostics payload schema `1` is unchanged. No schema-1 facts decoder or deprecated alias remains.
- Removed the public `FenceCursorCompletionKind`, `FenceCursorContext`, and `CompletionContext` policy wrappers. Rust hosts should call `completion_for_snapshot` over typed snapshots; transport adapters should use the editor-owned `COMPLETION_TRIGGER_CHARACTERS` list instead of maintaining their own activation table. The Web transport advances to API `4` because editor-capable artifacts now expose that trigger descriptor as a required export.
- Removed the public core `EditorCompletionCandidate` / `EditorCompletionVocabulary` surface and `EditorSemanticFacts::completion_vocabulary`; editor-core now owns completion labels, details, and snippets, while core publishes `EditorFamilySemantics` plus typed expected-syntax slots. `EditorExpectedSyntaxKind::Operator` becomes `FlowchartOperator`; `DirectionValue` splits into `FlowchartDirectionValue`, `CardinalDirectionValue`, and `BlockDirectionValue`, with new `Directive`, `Frontmatter`, `ClassName`, `StyleValue`, and `InteractionAction` slots.
- Tightened prerelease analysis configuration JSON around one analysis-owned contract. Profile and severity values now require their documented lowercase spelling (`core`, `recommended`, `strict`; `error`, `warning`, `info`, `hint`), so case-folded/whitespace-padded values and the former `warn` alias must be replaced. `merman` and `analysis` wrappers must contain objects and are mutually exclusive with each other and with direct analysis fields. JSON resource limits accept mathematical integers from their owner minimum through `4294967295`; larger values must be reduced or omitted to use the host default.
- Advanced the prerelease LSP `merman/configSchema` response to version `2` because the typed `constraints` projection is now mandatory. Clients must negotiate version `2` instead of partially decoding the former response shape.
- Removed the stateful Rust `DocumentWorkspace` map and `DocumentAnalysisOutcome` wrapper. Editor hosts now call `analyze_document_snapshot_with_shared_text` or `analyze_document_context_with_shared_text` and own URI/version storage themselves. The cancellable context function preserves cooperative cancellation as the outer result and resource rejection as the inner result; no deprecated alias or compatibility cache remains. See the [unreleased upgrade guide](docs/release/UNRELEASED_UPGRADE_GUIDE.md).
- The Flutter/Dart package now uses `package_ffi` and Native Assets with Dart 3.10 / Flutter 3.38 minimums. Legacy Flutter plugin registrars and platform-specific CocoaPods, SwiftPM, Gradle, CMake, and desktop wrapper glue are removed; `Merman.open()` remains the default API and `openMermanLibrary()` is removed.

### Added

- Added the experimental public `@mermanjs/node` alpha package group for Node.js 22 and newer on macOS arm64/x64, Linux x64 glibc/musl, and Windows x64 MSVC. The root loader selects one exact-version native package and exposes deterministic static SVG plus metadata/layout operations without a postinstall downloader or browser-WASM fallback.

### Changed

- Native release recipes now follow each wrapper's callable interface instead of shipping one universal complete binary. This substantially reduces distributed dependency closures, replaces Flutter's duplicated platform packaging with one Native Assets matrix, and adds an explicit compressed-package budget before pub.dev publication.
- Flutter pub.dev releases use package-specific `flutter-v<version>` tags so an unpublished package version can be built from a reviewed commit without moving an existing workspace tag.
- Web and Node npm publishing now preflight existing registry integrity and tags, publish missing exact versions directly under the requested final tag, and place the default Web package or Node loader last; retries skip members that already match the verified manifest.

### Fixed

- Playground Mermaid.js comparison realms now preserve SVG label colors without letting page CSS override Mermaid output, and ZenUML's injected `MS Sans Serif` font remains isolated to the affected comparison instead of changing other examples.
- Native Assets eliminates Flutter's legacy Linux Windows-wrapper linkage and SwiftPM symlink packaging paths, while Apple dylibs use normalized install names and refreshed signatures before Flutter's final assembly. #55 #56 #57

## [0.8.0-alpha.5] - 2026-08-09

0.8.0-alpha.5 is a distribution and provenance follow-up to alpha.4. Rust, CLI, LSP, FFI, and WASM runtime contracts are unchanged from alpha.4; users already on alpha.4 can update the version without another code migration. Flutter, Python, Web, Android, and Apple users coming from alpha.3 should follow their package changelogs and the [alpha.3 to alpha.5 upgrade guide](docs/release/ALPHA3_TO_ALPHA5_UPGRADE_GUIDE.md).

### Changed

- Restored the ordinary single-tag release path after alpha.4 required an audited recovery descendant. Alpha.5 crates and GitHub binaries are generated from one canonical tagged source revision.
- Made release-facing Rust dependency examples use exact prerelease versions; source-only Git commands are explicitly labeled and require a reviewed full commit.
- Made partial publication recoverable without moving an accepted tag or republishing crates and artifacts that already reached their registry.

### Fixed

- Release verification now accepts valid PDF token spacing, LSP header forms, and notification ordering while retaining format signatures, bounded protocol framing, required responses, and real final-archive execution.
- CLI and LSP cargo-dist archives include the project changelog, dual licenses, third-party notices, exact third-party license texts, and the package README; CLI archives also retain generated completion and manual-page assets.
- Corrected release documentation that still described alpha.4 as unavailable after its crates and GitHub binaries had been recovered and published.

## [0.8.0-alpha.4] - 2026-08-09

0.8.0-alpha.4 completes Merman's Mermaid 11.16.1 language and rendering surface and gives CLI, editor, browser, Rust, and native-SDK users explicit product contracts. It is a deliberately breaking prerelease; users upgrading from alpha.3 can use the [alpha.3 to alpha.5 upgrade guide](docs/release/ALPHA3_TO_ALPHA5_UPGRADE_GUIDE.md) because alpha.5 does not change these runtime contracts.

The published alpha.4 surfaces are the Rust crates and official CLI/LSP GitHub binaries. The source tree also prepared the lockstep browser and native wrapper contracts, but alpha.4 packages were not published to npm, pub.dev, PyPI, or the Android and Apple artifact channels; their package-local alpha.5 changelogs remain the release notes for those surfaces.

### Breaking changes

- Updated the compatibility target from Mermaid `11.15.0` to `11.16.1`; integrations that retain semantic, layout, or SVG parity snapshots should regenerate them. The separate `mmdc` compatibility workflow remains pinned to `@mermaid-js/mermaid-cli@11.16.0`. #21 #45
- Replaced the advertised root CLI rendering surface with explicit `render`, `batch`, and `mmdc` workflows. Existing root `-i/-o` invocations remain permanent hidden aliases for `mmdc`; native `render` and `batch` now advertise `-f/--format`, while their hidden `-e` aliases warn during `0.8.x` and are removed in `v0.9.0`. Capability automation must also migrate to schema 2 / CLI contract 3 and read `descriptor.digest`.
- Replaced `@mermanjs/web` capability subpaths and raw `pkg/**` imports with standalone `@mermanjs/web*` packages. Runtime discovery now uses `runtimeCatalog()` instead of `bindingCapabilities()` / `selectedRegistryProfile()`; browser text measurement uses an owned `createBrowserTextMeasurementSession()`, and semantic tokens use `editorSemanticTokenDescriptor()` plus packed `Uint32Array` data.
- Replaced the single `assertSafeSvgForDom()` browser assertion with distinct opaque self-contained and navigable admissions. Manual mounts must retain the matching admission through `prepareSelfContainedSvgForDomMount()` or `prepareNavigableSvgForDomMount()`; both revalidate the actual parsed root, and forged objects, structured clones, source/tree substitution, and the wrong capability are rejected at runtime.
- Removed the historical Cargo and runtime registry profiles `full`, `tiny`, `core-full`, and `core_full`. Disable defaults when absence matters and select observable capability leaves such as `svg`, `layout-cytoscape`, `layout-elk`, `math`, `analysis`, or `ascii`.
- Replaced public Chrono date/time values with project-owned `CivilDate`, `CivilDateTime`, `UtcOffset`, and `OffsetDateTime` types. Replace `chrono::NaiveDate` inputs and the removed `LocalTimeZone` conversion helpers with the checked constructors and resolution methods described in the upgrade guide. #37
- Replaced the C and Flutter ABI 2 path with generated ABI 3, and replaced Android's legacy JNI-through-C-ABI path with direct JNI transport API 1. These native SDKs use Options JSON schema 2, opaque reusable engines, generic operation dispatch, typed missing-capability errors, and runtime capability/resource catalogs. Apple and Python use UniFFI binding API 3, whose resource error record includes the stable `cause` discriminator; callers must upgrade each generated wrapper with its matching native artifact. Generic UniFFI options now belong in `MermanOperationRequest.options_json`.
- Resource-limit binding payloads now distinguish `ceiling` from `arithmetic_overflow` through the structured `details.resource.cause` field. Binding consumers must preserve the discriminator instead of classifying failures from display text; JavaScript `actual` and `max` counts are safe `number` values or canonical decimal `string` values for wider `u64` inputs.
- Replaced `AnalysisResult` with sealed `AnalysisGeneration` values, explicit ready/rejected outcomes, policy-separated `AnalysisOptions`, parser-only facts schema 1, and cancellable caller-owned shared-source entry points. `SourceMap::line_starts()` / `source_arc()` are replaced by behavioral line queries and `shared_source()`; copy text only through the explicit `to_owned_text()` boundary. See the [Rust and embedding API migration](docs/release/ALPHA3_TO_ALPHA5_UPGRADE_GUIDE.md#rust-and-embedding-api-migration) section for exact replacements.
- Replaced low-level rendering entry points and independent layout/SVG service selection with `HeadlessRenderer`, one `RenderSession`, and operation-owned `RenderEnvironment` policy. Migrate viewport fields to `container_width` / `container_height`, construct the now non-exhaustive `LayoutOptions` from its defaults/builders, use descriptor-driven resource profiles, and remove legacy Manatee and Flowchart ELK backend selectors.
- Replaced Dugong's `layout` / `layout_dagreish` split with one transactional Mermaid-compatible pipeline and non-exhaustive `LayoutError` results. Graphlib, ELK, Dugong, and Manatee now use source-backed ordering and non-exhaustive work errors; direct ELK importers must carry source `model_order` where edge order is observable.
- Replaced `render_svg_resvg_safe{,_sync}` and `svg_resvg_safe()` with typed `render_resvg_compatible_svg{,_sync}` / `HeadlessRenderer::render_resvg_compatible_svg_sync()` and `finalize_resvg_svg()`. These return `ResvgCompatibleSvg` instead of an unbranded `String`; downstream raster consumers must preserve that sealed value until the final byte boundary.
- Preserved `StateDiagramRenderLinks::{One, Many}` in `StateDiagramRenderModel::links`: Mermaid 11.16's state parser passes a fresh `idStatement` object for each `click`, so repeated declarations remain in source order and render as nested anchors. Consumers must handle both the single-link and repeated-link forms; `tooltip` remains the upstream-compatible string value.
- Replaced the prerelease `HostThemeProfile` API and `host_theme` Options JSON group with four independent owners: `Presentation::with_theme(...)`, `Presentation::with_profile(...)`, top-level `site_config`, and `with_svg_pipeline(...)` / `svg`. The old Rust render helpers and host-theme preset discovery methods were removed directly rather than retained as deprecated aliases; use the artifact-aware `presentation-catalog` metadata payload for discovery.
- Replaced the alpha.3 embedded LSP entry points with ordered `MermanLspService`, a one-time `MermanClientSocket::split()`, and transport-owned scheduling. Enable the `stdio` feature explicitly for the bundled server, send catalog/schema requests through the ordered service, and handle the exhaustive `StdioTermination::InputOverloaded` variant. #26 #33 #38
- Typst calls now always enforce the `constrained` resource policy; caller-provided trusted or unbounded profiles and numeric overrides are replaced at the plugin boundary.

### Added

- Added root-level `merman::render_svg` and `render_svg_with_id` one-shot facades for the default deterministic SVG workflow, with typed `RenderSvgError::NoDiagram` handling; `HeadlessRenderer` remains the configured and reusable path. Reworked the Rust example catalog into self-contained, task-named files with no shared support module.
- Added the `merman-export` crate and high-level Rust PNG, JPEG, and PDF rendering methods. Direct encoders accept the sealed `ResvgCompatibleSvg` artifact and expose output-specific raster/PDF plans and limits.
- Added completed-operation `layout_work_units()` evidence so hosts can calibrate public work ceilings from the same deterministic owner accounting that enforces them.
- Added source-backed parser, editor, typed-layout, and SVG coverage for all 35 Mermaid 11.16.1 diagram families, including Cynefin, Railroad, Swimlane, and Wardley. #21 #22 #23 #24 #40 #45
- Added a generated 35-family Playground example catalog with source provenance and search, plus isolated Compare and Bench realms that expose explicit failures and downloadable local benchmark evidence.
- Added deterministic Bash, Elvish, Fish, PowerShell, and Zsh completion assets plus recursive command man pages to release archives; cargo-binstall maps to project-built archives, and the repository also carries a reusable Nix source package and stable-only Scoop/WinGet submission candidates.
- Added semantic editor-theme presets, the `merman-modern` presentation profile, artifact-aware presentation discovery, and independent presentation-profile selectors in first-party CLI, Web, Playground, and Typst surfaces. Thanks @lovasoa for the original modern ELK profile in #28.

### Changed

- Split browser delivery into lockstep full, render, analysis, editor, and ASCII packages, and rebuilt the Playground around isolated runtime, editor, compare, and benchmark lifecycles.
- Separated oversized-output policy by format: SVG retains vector-specific limits, PNG/JPEG preflight final pixmap and embedded-image budgets, PDF owns page and filter-bitmap budgets, and Markdown batches reserve aggregate scheduling weight. The CLI exposes scoped raster, PDF, embedded-image, and batch-concurrency controls.
- Centralized built-in SVG root emission around source-backed bounds, responsive/fixed sizing, accessibility metadata, escaping, and deferred finalization. #22
- Made published crates and native/Web artifacts carry exact third-party notices, deterministic dependency projections, package-content verification, source provenance, checksums, and scoped release credentials; parser, renderer, SVG-pipeline, and C-ABI fuzz targets now run under sanitizer CI. #25
- Made Android source builds reproducible through the checked-in Gradle Wrapper and pinned Java 17, AGP, NDK, and Dokka tooling, with AAR, POM, native-slice, source, checksum, and API-documentation verification.

### Performance and footprint

- Substantially reduced the lint/analysis CLI binary and dependency closure compared with alpha.3, and removed repeated effective-config, label-measurement, hierarchy, ordering, conflict, and positioning work from common render paths. Complete products cover a broader capability set and are not uniformly smaller or faster; see the [refactoring evidence report](docs/release/ALPHA3_TO_ALPHA5_REFACTORING_REPORT.md) for scoped measurements. #48

### Fixed

- Closed `resvg-safe` non-navigation resource access: structural references are limited to same-document fragments, ordinary images require approved decodable inline raster data URLs, and PNG/JPEG/PDF exporters continue to disable string-href resolution independently.
- Fixed Mermaid 11.16.1 rendering and parsing edge cases across Flowchart and State self-loops, Sequence blocks and wrapping, TreeView icons and bounds, centered Railroad choices, Cynefin inline syntax, Packet accessibility descriptions, Architecture ids and seeds, XYChart labels, Pie highlighting, and Ishikawa structure. #21 #22 #23 #24 #40 #45
- Applied Mermaid's staged theme calculation consistently across Radar, Kanban, Mindmap, Timeline, Cynefin, and QuadrantChart, including font-only and partial overrides and valid resvg-safe fallbacks.
- Fixed Block `classDef` styles and connectors at browser-visible shape boundaries, projected browser `screen.availWidth` separately from container geometry for C4 parity, and enforced Flowchart limits before Dagre/ELK/Swimlane dispatch.
- Fixed Gantt date parsing at UTF-8 character boundaries and preserved target-date daylight-saving semantics without overflow at fixed-offset boundaries. Thanks @daikisuyama for #29.
- Fixed top-down ASCII routing for nodes on the same rank. Thanks @llimllib for #47.
- Calibrated the `interactive` layout-work ceiling to 800,000 owner-accounted units while leaving the stricter `constrained` profile unchanged. Hosts that intentionally render larger trusted workloads can select `trusted-native` or `unbounded-for-trusted-input` with appropriate outer isolation.
- Fixed `initMerman({ wasm })` to accept a URL, `Response`, byte buffer, or compiled `WebAssembly.Module`; callers no longer need wasm-bindgen's deprecated `{ module_or_path }` envelope.
- Prevented valid diagram ids from colliding with deferred root `viewBox` or `max-width` placeholders during SVG emission.
- Fixed Playground Event Modeling examples with payload line breaks by revalidating mounted SVG through XML serialization; any future mount rejection now stays inside the affected preview instead of blanking the app.
- Restored sanitized Kanban ticket navigation in the Playground and improved persistent workspace state, generated example search, keyboard and focus behavior, responsive viewport sizing, and touch gesture handling.
- Made LSP capability negotiation accurate for snippets, markup, diagnostics, code actions, semantic tokens, and document-only symbols. Stdio now keeps protocol stdout clean, prioritizes `OutputClosed` when output failure races another termination, rejects recoverable request overload with JSON-RPC `-32099`, and terminates when saturation would lose input integrity. #26

### Known limitations

- The in-process Node transport remains a private, unadmitted candidate; Node.js and SSR users should continue to invoke `merman-cli` as a subprocess. Local N-API versus Node-WASM measurements remain engineering evidence, not an alpha.4 package claim.
- Final same-host alpha.3 comparisons for complete and minimal SVG products must be refreshed against the release tag; browser-WASM throughput also lacks an equivalent-contract Mermaid.js comparison.
- Typst remains on an independent release track, and repository manifests do not prove that an alpha.4 package or artifact is already available from every declared registry or release channel.

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

- Hardened `resvg_safe` SVG cleanup for raw SVG and rendered icon fragments by stripping active SVG elements, event-handler attributes, unsafe URL attributes, and unsafe style/presentation `url(...)` values while preserving same-document fragment paint/reference URLs and raster data images.
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

- Carries forward the 0.7 prerelease API changes: detector construction uses `for_pinned_mermaid_baseline()`, known-type parser methods use `*_with_type*`, raster sizing uses the new target-aware `RasterOptions`, and theme metadata APIs use the supported-theme naming.

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
