# Changelog

All notable changes to this project will be documented in this file.

The format is based on *Keep a Changelog*, and this project adheres to *Semantic Versioning*.

## [0.8.0] - 2026-10-05

This `0.8.0` source candidate summarizes changes since **0.7.0 (2026-06-09)**, including seven alphas. It covers the final Rust, CLI, editor, and SDK contracts rather than intermediate prerelease APIs. The candidate targets Mermaid `12.1.0`; **`0.8.0` is not yet published**. Final release checks must run against the exact source selected for publication.

### Choose your upgrade path

| Your integration | Start here |
| --- | --- |
| Upgrading directly from stable 0.7.0 | [Stable upgrade guide](docs/release/V070_TO_V080_UPGRADE_GUIDE.md); it consolidates the required actions without seven separate migrations. |
| Rust parser, renderer, or editor host | [Rust embedding guide](docs/rendering/RUST_EMBEDDING.md) and [feature selection](docs/FEATURES.md), including a Zed-style family allowlist. |
| Already using alpha.7 | [Alpha.7-to-0.8.0 guide](docs/release/ALPHA7_TO_0_8_0_UPGRADE_GUIDE.md) for the remaining changes, including low-level ELK initializers. |
| Earlier alpha, browser, or native SDK | [Versioned upgrade index](docs/release/README.md) and the [package-specific contracts](docs/release/V070_TO_V080_UPGRADE_GUIDE.md#bindings-and-package-channels). |

Published alpha sections below remain historical records. Their intermediate feature names, APIs, measurements, and publication status do not override the stable candidate's contracts below.

### First things to check

- **Rust dependencies:** defaults now include SVG and ELK. For a copyable dependency declaration that excludes compiled ELK, see [Cargo features](README.md#cargo-features); choosing Dagre at runtime does not remove ELK from the build.
- **Rendering and presentation:** replace legacy renderer calls with typed requests; Mermaid 12 changes layout and appearance defaults. Review [breaking changes](#breaking-changes) and [presentation migration](docs/release/V070_TO_V080_UPGRADE_GUIDE.md#presentation-and-snapshots) before accepting new snapshots.
- **Distributed packages:** browser/native bindings and protocol schemas require matching artifacts, and their channels publish independently. Check the [package migration table](docs/release/V070_TO_V080_UPGRADE_GUIDE.md#bindings-and-package-channels) and the owning registry before upgrading.

### Breaking changes

- **Rust dependencies and diagram selection:** `merman` now defaults to `all-diagrams + complete-svg-elk` (SVG, Cytoscape, ELK, math). Parser-only or no-ELK consumers must disable defaults and explicitly select `all-diagrams` or required `diagram-*` families; output and layout features do **not** select languages. Replace `render` with `svg`, `raster` with `png` / `jpeg` / `pdf`, and `ratex-math` with `math`. Remove the obsolete `large-features`, `full`, `tiny`, and `core-full` presets. Family-owned model types and enum variants are conditional; see the [feature recipes](docs/FEATURES.md). #149
- **Rendering and export APIs:** migrate `HeadlessRenderer`, `HeadlessAsciiRenderer`, source-to-output convenience methods, and prerelease root `render_svg*` helpers to `Renderer`, typed `RenderRequest` / `RenderTarget` values, `SemanticArtifact`, and caller-owned `OperationControl`. CPU-bound rendering no longer has an `async fn` wrapper; hosts own scheduling and cancellation. Keep engine configuration reusable and output settings request-local, and preserve sealed SVG/export artifacts until their documented byte boundary. See the [embedding recipe](docs/rendering/RUST_EMBEDDING.md) and [symbol mapping](docs/release/UNRELEASED_UPGRADE_GUIDE.md). #62
- **Layout, theme, and SVG snapshots:** the Mermaid baseline moves from `11.15.0` to `12.1.0`. Flowchart, State, Class, ER, Requirement, Usecase, and Agentflow default to ELK when compiled; supported families also adopt Redux/Neo presentation. Replace family-local `defaultRenderer` with top-level `layout`. Explicit Dagre/default/classic settings retain those choices, not historical pixels or SVG bytes. Review geometry, IDs, DOM selectors, and fonts. #112 #154
- **ELK in default source builds:** default facade, CLI, and Rustdoc builds now include the EPL-2.0 `merman-elk-layered` implementation; Merman's MIT OR Apache-2.0 license is unchanged. To omit it, disable defaults and select families plus `complete-svg` or narrower features as shown in the [README](README.md#cargo-features). Runtime Dagre selection cannot remove compiled ELK; check feature unification and [artifact notices](THIRD_PARTY_NOTICES.md). #154
- **Fonts, dates, and runtime policy:** vendored font metrics are removed in favor of font-agnostic deterministic measurement or an explicit host callback. Replace public Chrono values with `merman::time::{CivilDate, CivilDateTime, UtcOffset, OffsetDateTime}`, including `Engine::with_fixed_today` inputs. The Rust runtime defaults to deterministic values; hosts needing current time, local time zones, or randomness must both compile the relevant `system-*` adapters and select the runtime policy. See the [stable migration steps](docs/release/V070_TO_V080_UPGRADE_GUIDE.md#rust-applications). #37
- **CLI scripts:** use explicit `render`, `batch`, or `mmdc` workflows. Root `-i/-o` calls remain permanent hidden `mmdc` aliases; native `render` / `batch` use `-f/--format`, with deprecated `-e` aliases warning during 0.8.x and scheduled for removal in 0.9.0. Stdout contains requested output bytes and diagnostics use stderr. Exit codes distinguish invalid input/config/output (`2`), direct I/O failures (`3`), and runtime/render failures (`1`); broken stdout pipes succeed. Network icon loading requires `--allow-network`, even though network support remains compiled by default. #19
- **Structured and editor consumers:** update analysis facts and config-schema consumers to version `2`, CLI contract consumers to `5`, and ASCII report consumers to `3`. ASCII reports distinguish requested/effective layouts, output encoding, semantic coverage, and structured-text fallback. Editor hosts use immutable parser-backed snapshots and Tree-sitter-owned syntax highlighting, while retaining document storage, revisions, and scheduling. See the [contract table](docs/release/V070_TO_V080_UPGRADE_GUIDE.md#bindings-and-package-channels); these version numbers identify different contracts. #61 #73 #88 #102 #134
- **Web and native SDKs:** replace browser capability subpaths / raw `pkg/**` imports with standalone `@mermanjs/web*` packages and use the explicit self-contained or navigable DOM-admission API when mounting SVG manually. Upgrade wrappers and artifacts together: C/Flutter use ABI `3`, Apple/Python UniFFI API `7`, Android direct JNI transport API `2`, and Web/WASM transport API `5`. Apple replaces hand-written C bindings with generated UniFFI; Android replaces the C-forwarding JNI bridge; Flutter moves to Native Assets with Dart 3.10 / Flutter 3.38 minimums. Options JSON uses schema `2`. Consult the [package migration table](docs/release/V070_TO_V080_UPGRADE_GUIDE.md#bindings-and-package-channels) for API replacements and capability differences.
- **Direct implementation-crate consumers:** update public model fields, exhaustive matches, and low-level layout constructors rather than relying on facade migrations alone. ELK edges retain terminal identities and multiple labels; the [alpha.7-to-0.8.0 guide](docs/release/ALPHA7_TO_0_8_0_UPGRADE_GUIDE.md#update-direct-low-level-elk-rust-consumers) lists the final initializer changes. Custom artifact-recipe readers must adopt schema `2` of `artifact-profiles-v2.json`, including logical parser IDs in `expected.diagram_families`. #149

### Added

- Added diagram support across the release line for Cynefin, Railroad dialects, Swimlane, Wardley, Agentflow, and Usecase, alongside the newer Mermaid shapes and syntax. Agentflow and Usecase originate in Mermaid 12.0.0; Agentflow remains beta syntax and neither has ASCII output. Parser, SVG, ASCII, and editor support are separate contracts; use the installed runtime catalog rather than a single family count. #21 #23 #24 #40 #45 #154
- Added **`merman-analysis`** for render-free diagnostics/lint and Markdown/MDX analysis, **`merman-editor-core`** for protocol-independent editor behavior, and **`merman-lsp`** for a ready-to-run server. Native and browser editor paths use the independently versioned Tree-sitter grammar for incremental highlighting. The VS Code integration adds previews, source actions, navigation, rename, and export. #20 #26 #73
- Added **`merman-export`** for resource-bounded PNG, JPEG, and PDF encoding; **`merman-layout-elk` / `merman-elk-layered`** for the Mermaid adapter and ELK implementation; and **`merman-doc`** for shared Rustdoc discovery and HTML wrapping. Most applications can continue through the `merman` facade rather than depending on every implementation crate. See [package selection](docs/release/PACKAGE_SURFACES.md).
- Added `merman-cli rustdoc build/check` for checked SVG fragments that consumers can commit and include without a renderer dependency. The existing `merman-rustdoc` macro gains inherited options, backgrounds, isolated IDs, resource policies, and Markdown-aware discovery; choose the [authoring workflow](crates/merman-rustdoc/README.md#choose-between-the-two-rustdoc-paths) appropriate to the consumer graph. #65 #131 #158
- Expanded terminal output with grapheme-aware plans, terminal-width profiles, canonical/compact layouts, styled encodings, structured-text projections, and bounded Flowchart/Sequence `auto` layout with one Compact retry before overflow handling. #13 #17 #88 #102 #134
- Added `DeterministicTextMeasurer::with_width_callback(...)` for full-string measurement using the host's display font while Merman owns wrapping and geometry; the [monospace example](crates/merman/examples/render_svg_monospace.rs) demonstrates integration. #140
- Added the experimental Node.js 22+ `@mermanjs/node` package group, explicit Node-targeted WASM transport, and the separately versioned Typst integration backed by `merman-typst-plugin`. The Node recipe supports deterministic SVG/layout, not the full Rust facade capability set; neither transport silently falls back to browser WASM. #96
- Added Linux ARM64 GNU CLI/LSP archives, shell completions and man pages, plus Playground example search, share URLs, export previews, independent preview modes, and an infinite canvas. Package channels and the VS Code / Typst / Tree-sitter versions remain independent. #4 #66 #67 #68 #128

### Changed

- Rendering, analysis, and first-party bindings share operation-scoped cancellation, deadlines, resource admission, and structured failure details. Graphical requests inherit source/model limits from their SVG environment when no explicit input policy overrides them. The general `interactive` layout-work allowance is 14,100,000 units; CLI and Rustdoc use finite `trusted-native` defaults with 15,000,000 units. Units are not a deadline or memory guarantee; preview hosts must cancel obsolete work and bound concurrency. The CLI interactive scheduling pool is 1 GiB. #62 #158
- Default Android, Apple, Python, and Flutter artifacts include SVG, Cytoscape/ELK, ASCII, analysis, validation, and document analysis, while omitting math, PNG/JPEG/PDF, and native runtime adapters. Discover capabilities from the loaded artifact and use a matching custom build when necessary; Rust defaults do not define SDK contents.
- Kept both network icons and the Rustdoc authoring command in default CLI builds and official archives after reviewing their costs. Narrow builds can select different capabilities; the [comparison report](docs/release/V070_TO_V080_RELEASE_REPORT.md) separates executable size, installed footprint, and cold-process startup evidence.
- Diagram-local colors and fonts pass through a common CSS-value boundary. Safe `themeVariables` and fonts work from directives/frontmatter, while host security policy still controls `themeCSS` and locked fields. Presentation profiles, Mermaid themes, host site config, and SVG pipelines have separate owners. #28 #138

### Fixed

- Made unsupported-parser diagnostics suggest the owning `diagram-*` or `all-diagrams` feature for known built-ins, while preserving structured codes and unknown/custom failures.
- Added Rustdoc resource options, per-diagram/per-theme budgets, inheritance, and actionable limit errors. PNG/JPEG/PDF scheduling reserves the larger layout or encoding phase rather than adding non-overlapping workspaces. See the [resource-policy guide](docs/integration/RESOURCE_POLICY.md). #158
- Corrected C4 relationship endpoints for people, components, databases, queues, and nested boundaries, including self-relations and near-overlapping centers.
- Aligned ELK feedback/cyclic routing, nested frames, fork/join direction, group-title wrapping, terminal labels, and container paint order with Mermaid 12. Class, ER, Requirement, and Usecase honor `elk.lineHops` without changing node/label placement. #154 #164
- Respected `htmlLabels: false` for Class, ER, and Requirement labels, kept Class lollipop interfaces outside namespaces, and left-aligned Requirement body text under ELK. Corrected Flowchart stadium sizing with HTML/fractional-font metrics and unified label measurement across affected shape paths. #159 #160
- Mirrored Packet fields and bit labels for descending bit order, and centered XYChart titles over the plot area while hiding titles that cannot fit.
- Corrected Unicode/keyword/cardinality boundaries, merged repeated Flowchart subgraph IDs without self-membership, and retained Sequence actor configuration and keyword-like IDs. Duplicate Eventmodeling frame IDs now fail with original source locations; reset-frame relations and Usecase EOF diagnostics preserve source semantics. #146 #147 #153
- Preserved derived theme values and generated sibling keys during partial overrides across all 11 themes, restored Usecase Neo shadows/notes/gradients, and retained explicit ER styling in Redux themes. #138
- Included the suppressed parse-error message in error diagrams with bounded Unicode wrapping, and shared one linear frontmatter scanner between detection and extraction.
- Corrected Journey raster output, Gantt UTF-8 date parsing and exclusion/date-boundary handling, and exported-font matching with Unicode case folding. Improved ASCII routing, relation labels, direction, and compound/Sequence layout. #6 #13 #17 #29 #47 #113 #134
- Kept RoughJS geometry independent of CSS color spelling, prevented malformed Unicode color values from panicking, and fixed XML text type inference in integrated graphs such as Zed. #136
- Made Playground editor sizing, gallery filters/position, reduced motion, tooltip delay, and queued preview refreshes behave consistently. Comparison styles and injected fonts no longer leak into other previews. #2 #72 #80 #84

### Security

- Hardened configuration/CSS admission, icon/SVG sanitization, and non-navigation resource handling in export-safe SVG. Binary exporters retain independent external-resource restrictions and format-specific budgets; browser DOM insertion requires its own admission rather than treating any exported SVG as safe to mount.
- Updated dependencies including `rustls 0.23.45` (RUSTSEC-2026-0285), `js-yaml 4.3.2` (GHSA-2883-xcg3-v3hh), and DOMPurify `3.4.16` (GHSA-p98j-92pf-mc4p). Reference rendering now verifies the selected dependency graph rather than executing stale embedded dependencies; sanitizer-default generation rejects unsupported input. See the published alpha sections for advisory-specific history. #10 #11

### Performance and footprint

- On six selected Flowchart inputs with explicit Dagre/classic settings, complete native `merman` library SVG rendering at `01cc4562f` showed approximately 15%-33% lower paired-median time than `v0.7.0`. These are Windows-host observations: both A/A noise calibrations were inconclusive, so they do not establish a statistically confirmed or whole-library speedup. The comparison excludes CLI startup, ELK, and alpha.7; see the [matched-Dagre report](docs/performance/library_dagre_v070_to_01cc4562f_2026-10-05.md) for settings, individual results, and limitations.
- Complete CLI products also deliver more capabilities and are larger than the previous stable binaries. The [release comparison report](docs/release/V070_TO_V080_RELEASE_REPORT.md) separates published historical archive measurements from candidate feature-cost experiments; it is not a final 0.8.0 artifact-size guarantee. Earlier alpha measurements retain their own sources, recipes, and hosts.

### Availability and known limitations

- 0.8.0 is not yet released. Cargo crates, CLI/LSP archives, Web, Node, Flutter, Python, Android, and Apple publish through separate channels; a workspace tag does not certify all of them. Follow [package surfaces](docs/release/PACKAGE_SURFACES.md) and the matching package changelog. Node remains experimental, and Typst / Tree-sitter / VS Code versions are independent.
- Deterministic text measurement does not load the final display font; browser metrics, HTML labels, and hand-drawn rendering retain documented parity boundaries. Hosts needing exact display-font measurement should provide their own callback. The broader reusable-theme refactor remains deferred beyond 0.8.0.
- Performance observations cover selected native operations, not every family, scale, or transport; unresolved signals remain recorded in the [performance backlog](docs/performance/PERF_PLAN.md). Final release admission is tied to the exact tagged source, including documentation changes.

### Contributors

Thanks to @betyourluck, @can1357, @colindean, @daikisuyama, @Ginger-Beard, @ixmoyren, @llimllib, @lovasoa, @lucaschoeneberg, @makenowjust, @manixate, @mrueg, and @vlasky for contributions across the 0.8 release line, and to @aurabindo for the Flowchart browser-clipping report. Published alpha entries retain the contribution-specific history.

## [0.8.0-alpha.7] - 2026-09-30

Alpha.7 is planned as the final alpha in the 0.8.0 release cycle. It moves the compatibility baseline to Mermaid 12.0.0, adds Agentflow and Usecase, and makes diagram-family selection explicit for custom Rust builds. This is a breaking prerelease; see the [alpha.6 to alpha.7 upgrade guide](docs/release/ALPHA6_TO_ALPHA7_UPGRADE_GUIDE.md) before updating source dependencies, generated bindings, or snapshots. Package channels publish independently.

### Breaking changes

- Updated the compatibility baseline from Mermaid `11.17.2` to `12.0.0`. Flowchart, State, Class, ER, Requirement, Usecase, and Agentflow now default to ELK when that capability is compiled in; Mermaid 12 appearance defaults also change supported families to `redux-color` and `neo`. Refresh layout/SVG snapshots, replace removed family-local `defaultRenderer` selectors with top-level `layout`, and explicitly select `layout: dagre`, `theme: default`, and `look: classic` when the previous presentation is required. #154
- The default `merman`, `merman-cli`, and `merman-rustdoc` source builds now include the EPL-2.0 `merman-elk-layered` implementation. Merman's own MIT OR Apache-2.0 license is unchanged. To omit ELK from a Rust dependency graph, use `default-features = false` with the required diagram families and `complete-svg` or narrower capability leaves, and check feature unification across the application. Selecting Dagre at runtime does not remove the compiled ELK dependency. Existing full prebuilt artifacts already included ELK; review the [third-party notices](THIRD_PARTY_NOTICES.md) for the selected artifact. #154
- Low-level crates and `default-features = false` consumers now require positive `diagram-*` selectors. Add `all-diagrams` to retain the previous parser surface, or select only required families. Family-exclusive public model types and enum variants are conditional. Default facade, CLI, and Rustdoc builds retain all families; `complete-svg` selects outputs and engines without selecting languages. #149
- `merman-rustdoc` now defaults to all families, SVG, Cytoscape, and ELK without math. Add `features = ["math"]` when upgrading diagrams that relied on alpha.6's math-enabled default. Macro SVG backgrounds are transparent by default; set `background = "white"` for an opaque canvas. Generated IDs include occurrence and theme identity, so regenerate snapshots. #131 #154
- ASCII reports advance to schema `3` with requested/effective layouts and Compact-attempt information. Direct Python/Apple UniFFI bindings advance to API `7`; regenerate wrappers and native libraries together. The C ABI remains `3`, Android transport API remains `2`, and Web/WASM transport API remains `5`. #134
- Custom artifact-recipe readers must adopt `artifact-profiles-v2.json` schema `2`, including exact logical parser IDs in `expected.diagram_families`. Runtime capability IDs and `feature-surface-v1.json` are unchanged. #149

### Added

- Added native Agentflow and Usecase parsing, semantic models, layout, and SVG rendering, plus Playground examples, Tree-sitter highlighting, lint/diagnostic integration, and LSP/editor navigation and rename. Agentflow diagnostics cover removed shapes, unsupported shapes, and cyclic containment. Neither family has ASCII output; Agentflow follows upstream beta syntax, and body completion remains limited. The independently versioned `tree-sitter-mermaid` package moves to `0.2.0`. #154
- Added Linux ARM64 GNU CLI and LSP release targets with final-archive execution checks on native ARM64 runners. Thanks @mrueg for #128.
- Added `DeterministicTextMeasurer::with_width_callback(...)` so Rust hosts can measure complete strings in their display font while Merman owns wrapping and geometry. A monospace example demonstrates font selection and shaping. Thanks @manixate for #140.
- Added opt-in ASCII `auto` layout for bounded Flowchart and Sequence output, with one Compact retry before the selected overflow policy. Flowchart Compact uses a smaller default horizontal rank gap while retaining explicit spacing overrides. #134
- Added Rustdoc macro options `background`, `id_prefix`, and `inherit`, plus documented `build.rs` directory tracking for `.mmd` edits, additions, and deletions. The new `merman-doc` crate shares discovery and HTML wrapping between the macro and CLI without a renderer or file-access dependency. #131

### Changed

- Coupled prerelease crates now require exact sibling versions so fresh Cargo resolution cannot mix APIs from different prereleases. Upgrade the coupled graph together; independently versioned packages retain their own compatibility requirements.
- Diagram-local colors and typography now pass through one CSS-value admission boundary for init directives and YAML frontmatter. Safe `themeVariables` and fonts work by default; `themeCSS` remains host-controlled, and host `secure` policies can lock presentation fields. #138
- Rustdoc diagram discovery follows Markdown structure in lists, blockquotes, footnotes, and nested combinations. Standalone includes require explicit container prefixes and JSON-compatible quoted paths. Duplicate macro options report errors; strict embedding rejects unsafe resources while preserving supported HTML labels and navigation links. #131
- LSP semantic-token work uses bounded workers and reuses unchanged syntax line indexes within a per-session budget. Rendering avoids repeated image-label scans and discarded SVG serialization during pure-math measurement. These changes do not imply an across-the-board speedup after the new Mermaid defaults. #139
- ASCII graph maps use faster hashing for internal numeric keys. Thanks @makenowjust for #133.
- Playground, CLI help, and SDK guides clarify that `readable` SVG is an advanced text-fallback mode; the default Mermaid SVG preview and pipeline values are unchanged. #134

### Fixed

- Corrected Mermaid keyword, Unicode whitespace, identifier, and ER cardinality boundaries, including names beginning with cardinality words, the entity name `direction`, and parent-cardinality tokens. Flowchart, Class, and State parsing now follows upstream character ranges rather than a generic Unicode-word approximation. Thanks @betyourluck for #146 and #153; additional coverage landed in #147 and #154.
- Fixed `base` theme overrides leaving derived Sequence, State, Gantt, and Pie colors at their defaults. Theme calculations now preserve intermediate and explicit derived overrides. #138
- Fixed XML text type-inference failures in integrated dependency graphs such as Zed without changing SVG validation policy. Thanks @lucaschoeneberg for #136.
- Preserved horizontal ASCII/Unicode edge segments around labels, including bidirectional arrows, and corrected even-width label centering. #132 #134
- Fixed Rustdoc handling of conditionally disabled items, renamed Cargo dependencies, inherited options, comments, code examples, mixed documentation attributes, and blank lines inside SVG text or CSS. Isolated SVG IDs and built-in styles across embedded diagrams, including Event Modeling, Ishikawa, TreeView, ZenUML, and Error output. Identical methods from one declarative-macro call still need distinct `id_prefix` values or tree annotations on complete impl blocks. #131
- Aligned Flowchart and State ELK edge ports, routing, terminal alignment, and line hops with Mermaid 12, and corrected Mermaid 11.17 Flowchart behavior carried forward into this release. #120 #154
- Updated vulnerable dependencies, including `rustls 0.23.45` for TLS 1.3 handshake encryption-level validation (RUSTSEC-2026-0285) and `js-yaml 4.3.2` for merge-key CPU exhaustion (GHSA-2883-xcg3-v3hh).

### Known limitations and next steps

- The broader theme refactor announced in alpha.6 is deferred to a future release because the remaining implementation work, code-size impact, and performance concerns need more time. Alpha.7 includes the Mermaid 12 theme defaults and targeted theme fixes above, but does not complete the planned reusable theme system across first-party surfaces. Stabilization is the next focus for the 0.8.0 line.
- Deterministic headless text measurement does not load the display font. Browser font metrics can still overflow some calculated bounds, including the documented Flowchart title case. Hosts requiring display-font measurement should supply their own measurement callback; see the [font-boundary report](docs/alignment/MERMAID_12_TITLE_029_FONT_BOUNDARY_2026_09_30.md).

## [0.8.0-alpha.6] - 2026-09-02

Alpha.6 is a deliberately breaking prerelease and the first 0.8 release aligned with Mermaid `11.17.2`. It unifies operation control across Rust and first-party bindings, expands terminal and editor workflows, and brings the native, Web, Node.js, Flutter, Python, Apple, Android, and Typst surfaces onto explicit contracts. The workspace crates and CLI/LSP archives are published from immutable tag `v0.8.0-alpha.6`; Web, Node.js, Flutter, Python, Apple, Android, and Typst remain independent publication tracks whose availability must be checked separately. See the [alpha.5 to alpha.6 upgrade guide](docs/release/ALPHA5_TO_ALPHA6_UPGRADE_GUIDE.md).

### Highlights

- Mermaid compatibility now follows the `11.17.2` behavior graph, including ER subgraphs, collapsed Flowchart subgraphs, new Flowchart shapes, XYChart legends, and updated C4, Class, Requirement, and ELK rendering contracts.
- Rust and the first-party bindings now share one operation-scoped execution model with explicit cancellation, deadlines, resource policies, and structured failure details.
- Terminal rendering now supports grapheme-aware plans, terminal-width profiles, styled encodings, broader family coverage, structured-text fallbacks, and configurable Flowchart label wrapping.
- Editor workflows now combine parser-backed semantic snapshots with Tree-sitter syntax highlighting, including incremental native LSP and browser-worker paths that avoid full analysis on every keystroke.
- New delivery surfaces include a Node-targeted WASM package for the experimental Node.js 22+ group, checked rustdoc SVG fragments, richer Playground exports and sharing, and the independently versioned Typst `0.3.0` package.

### Looking ahead

Barring unexpected release-critical findings, `0.8.0-alpha.7` is planned as the final alpha before `0.8.0`. It will be a focused pass on making theme-driven Mermaid styling easier to define, reuse, and apply consistently across first-party surfaces before the stable release.

### Mermaid compatibility baseline

Alpha.6 selects Mermaid `11.17.2`, `@mermaid-js/parser` `1.2.1`, and `@mermaid-js/layout-elk` `0.2.3` as one behavior graph. Regenerate compatibility, layout, and SVG snapshots when upgrading from alpha.5; the selected companion graph and source provenance are recorded in `tools/upstreams`.

### Upgrade first

The detailed contracts remain in **Breaking changes** below. Before mixing alpha.5 code, generated bindings, or package artifacts with alpha.6:

- Migrate Rust rendering callers to `Renderer`, typed requests and targets, `SemanticArtifact`, and caller-owned `OperationControl`.
- Regenerate analysis facts, language bindings, Web/WASM glue, Android JNI sources, and compatibility snapshots from the same alpha.6 source revision.
- Update editor and automation consumers to analysis facts schema `2`, config schema `2`, CLI contract `5`, and Tree-sitter-owned syntax highlighting.
- Refresh ASCII capability/output decoders and select the matching native artifact for Apple/Python binding API `6`, Android transport API `2`, or Web/WASM transport API `5`.
- Review ELK, Flutter, and Typst migrations: ELK is opt-in for complete SVG builds, Flutter requires Dart `3.10` / Flutter `3.38` with Native Assets, and Typst option precedence changed.

### Breaking changes

- Updated the compatibility target from Mermaid `11.16.1` to `11.17.2`, including ER subgraphs, collapsed Flowchart subgraphs, the `person`, `folder`/`directory`, `bucket`, `console`, and `browser` shapes, XYChart legends, unified C4/Class behavior, requirement edge identity, and the selected ELK DOM contract. Consumers that compare compatibility JSON, layout records, SVG structure, ids, or coordinates must refresh their expectations. #112
- Replaced `HeadlessRenderer`, `HeadlessAsciiRenderer`, root `render_svg*` helpers, public SVG preparation stages, and CPU-bound render `async fn` wrappers with one operation-scoped `Renderer`, typed `RenderRequest` / `RenderTarget` values, format-neutral `SemanticArtifact`, and cloneable `OperationControl` cancellation/deadline ownership. #62
- Updated typed render models for Flowchart markers and visibility, declaration-ordered ER entities, Gantt constraints, Sequence actor lifecycles, authored Mindmap ids, and repeated-section ownership; Rust struct literals and exhaustive matches must adopt the new fields and types.
- Advanced analysis facts to schema `2`, changed `DiagramParseOutcome::Parsed` to a struct variant with parser-owned warning facts, removed the Flowchart-only rich graph and stateful `DocumentWorkspace`, removed parser-emitted editor token APIs, and advanced `merman/configSchema` to version `2`. Editor integrations now use typed snapshots, `EditorFamilySemantics`, and Tree-sitter syntax ownership. #61 #73
- Advanced the machine-readable CLI contract from `4` to `5` and replaced ASCII's single grid ceiling with typed six-phase resource policies, terminal-width profiles, schema-2 output encoding, separate semantic/primary coverage, and the `structured_text_fallback` name. Consumers that decode capability records or compare terminal bytes must refresh them. #88 #90 #102
- Changed `merman` and `merman-rustdoc` `complete-svg` to include SVG, Cytoscape, and math without ELK. Select `complete-svg-elk` or `layout-elk` when ELK is required and distribute the corresponding EPL-2.0 notices and source provenance.
- Advanced Apple/Python UniFFI bindings to API `6`, Android JNI transport to API `2`, and Web/WASM transport to API `5`; regenerate each projection and deploy it with its matching native or WebAssembly artifact. Generic requests use `MermanOperationRequestV4` with optional `MermanOperationControl` values for cooperative cancellation and deadlines.
- Moved Flutter to Dart `package_ffi` and Native Assets with Dart `3.10` / Flutter `3.38` minimums, removing legacy plugin registrars, platform wrapper glue, and `openMermanLibrary()` while retaining `Merman.open()`.
- Changed Typst wrapper precedence: direct `diagram-id` / `id` overrides profile aliases, direct `site-config` replaces the profile object, and raw `options` bypasses shorthand validation. Conflicting aliases must be made explicit.

### Added

- Added production Tree-sitter highlighting to the native LSP and Playground through the canonical `tree-sitter-mermaid` grammar, portable query, and incremental browser worker; the grammar keeps independent Cargo/npm versioning. #73
- Added the opt-in `@mermanjs/node-wasm` package to the experimental `@mermanjs/node` group for Node.js 22+ deployments that cannot load a native addon. It is independent of `@mermanjs/web` and never downloads or silently selects another transport. #96
- Added grapheme-aware ASCII plans, canonical/compact Flowchart and Sequence layouts, ANSI16 semantic roles, schema-2 output encoding, structured-text projections for non-diagrammatic families, and configurable Flowchart label wrapping for Issue #53.
- Added `merman-cli rustdoc build/check` for checked static SVG fragments that can be committed, freshness-checked, and included by Rust documentation without adding a renderer or proc macro to the consumer graph. #65
- Added Playground export previews for SVG/raster/document outputs, reproducible share URLs, and an infinite-canvas viewport that keeps editor pan/zoom state separate from exported geometry. #66 #67 #68
- Published independently versioned Typst package `0.3.0` with Typst plugin ABI `2` and a smaller deterministic WASM closure after removing ICU collation data and generated font-metric tables; the Typst channel remains separate from the workspace release.

### Changed

- Decoupled Playground Tree-sitter syntax state from strict Merman semantic analysis so highlighting no longer triggers or waits for full analysis on every keystroke.
- Replaced one universal native release closure with wrapper-specific Android, Apple, Python, and Flutter profiles. Their default artifacts include SVG, Cytoscape/ELK layouts, ASCII, analysis, validation, and document analysis while omitting math, raster/PDF export, and native clock/time-zone/random adapters.
- Kept the native C ABI `3` prefix compatible with the alpha.5 header while higher-level generated bindings continue to reject mismatched transport versions.
- Made unavailable optional operations report typed missing-capability or unsupported-operation errors instead of silently returning incomplete results.

### Fixed

- Made deterministic SVG text measurement font-family agnostic and grapheme-aware, centered Flowchart edge labels from emitted text bounds, preserved empty Pie canvases, and applied Flowchart `diagramPadding` directly for zero and fractional values.
- Fixed native PNG, JPEG, and PDF text export to match font-family names with Unicode default case folding while preserving CSS family-stack order, so lowercase Mermaid defaults no longer bypass the requested font. Thanks @Ginger-Beard for #113.
- Restored Mermaid 11.17 Flowchart and Swimlane edge-path selectors and CSS classes, and aligned C4, Class, ER, and XYChart DOM/layout details with the selected upstream behavior graph.
- Restored `merman-rustdoc` browser-parity SVG defaults without duplicate visible fallback text; explicit `readable` and `resvg-safe` pipelines remain available. #81
- Restored `roughr-merman` 0.12 source compatibility for Merman 0.7 while preserving operation-owned randomness for current releases.
- Fixed Flutter Native Assets assembly across Linux, Windows, and Apple packaging, including dylib install names, signatures, and legacy wrapper removal. #55 #56 #57
- Fixed ASCII direction, compound ownership, routing, markers, notes, Sequence controls, XYChart axes, declaration order, structured disclosure, and viewport fallback accounting across the common families.
- Fixed long Flowchart label wrapping so cancellation is checked between measurement probes and no partial output is returned.
- Isolated Playground comparison styling so page CSS no longer overrides Mermaid label colors and ZenUML's injected font does not leak into other examples.
- Corrected target-width ABI discovery for packaged Flutter ARMv7 libraries.

### Availability and known limitations

- The workspace crates and CLI/LSP archives are published as `0.8.0-alpha.6` from immutable tag `v0.8.0-alpha.6`. The Typst `0.3.0` package, Web group, Flutter package, Python wheels, Apple XCFramework, Android AAR, and seven-package Node.js group are published independently from the alpha.6 source line. The Node group was manually bootstrapped from its verified package-group artifact, so its immutable alpha.6 npm tarballs have no npm provenance; later Node releases use Trusted Publishing.
- The default Android, Apple, Python, and Flutter profiles intentionally omit math, PNG, JPEG, PDF, and native runtime adapters. Use the runtime catalog or a custom artifact when those capabilities are required; ELK requires `complete-svg-elk` or `layout-elk`.
- The Node.js package group remains experimental and follows an independent publication track. The Typst `0.3.0` package is also independently versioned; neither channel should be inferred from the workspace version alone.

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
