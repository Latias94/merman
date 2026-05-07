# Changelog

All notable changes to this project will be documented in this file.

The format is based on *Keep a Changelog*, and this project adheres to *Semantic Versioning*.

## [Unreleased]

### Added

- Docs: add the `docs/workstreams/fearless-refactor/` workstream with roadmap, TODOs, milestones,
  and render-model inventory for the next cleanup-focused release.
- Docs: add a generated parity override footprint snapshot for fearless-refactor governance.
- Docs: add an override policy for text/render width compatibility data.
- Docs: add a post-migration sequence typed render-model performance spot-check.
- `xtask verify --strict`: add a strict refactor/release gate that includes `cargo fmt`,
  `cargo check --workspace --all-features`, workspace all-target/all-features Clippy with
  `-D warnings`, `cargo nextest run`, and SVG DOM parity checks.

### Changed

- Core/render pipeline: centralize typed render-model dispatch and suppressed error-diagram
  construction so public parse/render entrypoints share one fallback path.
- Core/render API: `parse_diagram_for_render_model_sync` is now the single render-optimized parse
  entrypoint; semantic JSON callers should continue using `parse_diagram_sync`.
- Sequence render pipeline: add a typed render model for layout/SVG render-model dispatch while
  keeping the semantic JSON parse API stable.
- Render text subsystem: split shared text types, deterministic width heuristics, Mermaid-like
  Markdown tokenization, Markdown HTML/XHTML label fragments, SVG/font bbox helpers, and flowchart
  HTML parity helpers into dedicated `text/*` modules, and move the `TextMeasurer` trait,
  flowchart-aware text metrics, and deterministic/vendored measurers into dedicated measurement
  boundaries while keeping existing `crate::text::*` callers stable.
- Render text overrides: add a `text/overrides.rs` lookup boundary for generated text override
  data and colocate flowchart override lookup tests with it.
- Timeline: colocate long-word SVG bbox override lookup and regression tests with the timeline
  layout owner.
- Class renderer: split edge ids/classes, geometry/order, edge label/terminal emission, and shared
  HTML label helpers out of `svg/parity/class/render.rs`.
- Class renderer: move class edge paths, edge labels, terminals, edge data-point encoding, and
  edge timing accumulation into `svg/parity/class/edge.rs`.
- Class renderer: move SVG content-bounds accumulation helpers into
  `svg/parity/class/bounds.rs`.
- Class renderer: move class node shell, basic-container emission, HTML row measurement, HTML
  label-group emission, SVG class node body emission, SVG title emission, SVG label-run emission,
  and divider emission into `svg/parity/class/node.rs`.
- Class renderer: move class node render-order/index construction into
  `svg/parity/class/namespace.rs`.
- Class renderer: move namespace wrapper/subgraph render-mode selection into
  `svg/parity/class/namespace.rs`.
- Class renderer: move namespace cluster group emission into
  `svg/parity/class/namespace.rs`.
- Class renderer: move root viewBox/max-width calibration and class diagram title positioning into
  `svg/parity/class/viewbox.rs`.
- Sequence renderer: start the actor/participant split by moving actor label emission into
  `svg/parity/sequence/actors.rs`.
- Sequence renderer: move pre-actor box/rect frame emission into
  `svg/parity/sequence/frames.rs`.
- Sequence renderer: move actor popup menu emission into `svg/parity/sequence/actors.rs` and share
  sequence node geometry helpers through `svg/parity/sequence/geometry.rs`.
- Sequence renderer: move actor-man top/bottom variant emission into
  `svg/parity/sequence/actors.rs`.
- Sequence renderer: split actor popup menu emission and actor-man variants into
  `svg/parity/sequence/actor_popup.rs` and `svg/parity/sequence/actor_man.rs`.
- Sequence renderer: move top/bottom actor box and lifeline emission into
  `svg/parity/sequence/actors.rs`.
- Sequence renderer: move SVG render settings/config parsing into
  `svg/parity/sequence/settings.rs`.
- Sequence renderer: move root SVG opening, accessibility title/description, and sequence viewport
  override handling into `svg/parity/sequence/root.rs`.
- Sequence renderer: move activation precomputation and group emission into
  `svg/parity/sequence/activation.rs`.
- Sequence renderer: move note emission into `svg/parity/sequence/notes.rs`.
- Sequence renderer: move message-prelude interaction overlay orchestration for notes,
  activations, and block frames into `svg/parity/sequence/interactions.rs`.
- Sequence renderer: move loop/alt/par/critical block model collection into
  `svg/parity/sequence/block_collection.rs`.
- Sequence renderer: move block label wrapping, loop text emission, and block frame range helpers
  into `svg/parity/sequence/blocks.rs`.
- Sequence renderer: split block label wrapping and loop text emission into
  `svg/parity/sequence/block_text.rs`.
- Sequence renderer: split block frame/message range geometry into
  `svg/parity/sequence/block_geometry.rs`.
- Sequence renderer: split actor label, lifeline wrapper, and non-actor-man shape emission into
  `svg/parity/sequence/actor_shapes.rs`.
- Sequence renderer: split actor-man glyph geometry and SVG emission into
  `svg/parity/sequence/actor_man_glyphs.rs`.
- Sequence renderer: move message label/line emission and autonumber handling into
  `svg/parity/sequence/messages.rs`.
- Sequence renderer: share block frame and label-box emission helpers across
  loop/alt/par/critical variants.
- Sequence renderer: share block message y-range and separator y-position helpers across
  loop/alt/par/critical variants.
- Sequence renderer: share single-section loop/opt/break block emission through a common helper.
- Sequence renderer: share multi-section alt/par block emission through a common helper.
- Sequence renderer: move critical block emission into the shared block module while preserving
  its Mermaid-specific multi-section frame widening and header-height behavior.
- Sequence renderer: add a block render context so loop/alt/par/critical frame helpers share one
  explicit parameter bundle instead of repeated long argument lists.
- Class renderer: move note node emission and note-specific render timing accounting into
  `svg/parity/class/note.rs`.
- Class renderer: move interface node emission into `svg/parity/class/interface.rs`.
- Class renderer: move namespace ordering and nested subgraph emission into
  `svg/parity/class/namespace.rs`.
- Class renderer: remove the duplicate namespace-subgraph edge path/label emitter and route it
  through the shared optimized edge group path.
- Class renderer: move SVG text wrapping, label bbox, and bold-width compensation helpers into
  `svg/parity/class/label.rs`.
- Class renderer: move render lookup maps, small config helpers, and timing detail emission into
  `svg/parity/class/context.rs`.
- Class renderer: move HTML class node body emission into `svg/parity/class/node.rs`.
- Render text tests: move markdown-only tokenization and label-fragment tests next to the split
  Markdown modules.
- `xtask report-overrides`: scan all generated override modules by category instead of relying on
  a hand-maintained file list.

### Removed

- Core/render API: removed the obsolete `parse_diagram_for_render_sync` compatibility API and its
  async alias, plus the old `mindmap` / `stateDiagram` JSON-for-render helper paths.
- Render feature flags: removed the stale `merman-render/flowchart_root_pack` experimental debug
  feature and its disabled post-layout packing code.

## [0.4.0] - 2026-03-12

### Added

- `xtask`: support custom fixture roots in SVG baseline generation/comparison, add Markdown-aware text measurement, and
  integrate an opt-in Node/Puppeteer KaTeX path when `tools/mermaid-cli` is available.
- Docs: add and expand `docs/workstreams/*` parity planning material, including root viewport (`parity-root`) checks and
  text-measurement alignment notes.
- Tests/Fixtures: add a broad parity corpus covering font-size precedence, HTML label wrapping, Markdown `<br/>`
  continuations, unknown XML entities, KaTeX flowcharts, text-style overrides, and root viewport probes across multiple
  diagram types.

### Changed

- Text parity work now consolidates large amounts of fixture-derived width/height/padding data into generated
  `*_text_overrides_11_12_2` tables instead of leaving diagram-specific literal branches inline across layout/render code.
- SVG/style precedence now follows Mermaid more consistently: `themeVariables.fontSize` and `themeVariables.fontFamily`
  win where upstream uses them, and parity tooling captures more text-style drift during SVG comparison.

### Fixed

- Text/Markdown: align shared HTML/SVG text handling with Mermaid for inline code, failed `__` delimiter runs,
  paragraph-vs-raw-block HTML labels, punctuation-heavy URL wrapping, hyphenated-token min-content width, and trailing
  whitespace height edge cases.
- Flowchart: align HTML/SVG label wrapping, class/style text application, entity decoding, edge-label DOM/background/root
  bbox behavior, and complete the upstream Cypress new-shapes strict-XML buckets.
- Class: reduce strict-XML drift across note labels, namespaces, generics, relations/cardinality terminals, style
  propagation, annotation-driven sizing, and SVG/HTML title/member width measurement.
- ER: align relationship-label Markdown/backtick handling, root `htmlLabels` semantics, and entity/root font-size
  precedence with Mermaid baselines.
- State/Class/Mindmap/Kanban/Architecture: align remaining HTML label widths, wrapping-width handling, shared text
  constants, width parsing, and icon/service label fallback geometry between layout and SVG render.
- Block: complete strict XML parity for the Mermaid block corpus and align remaining marker-aware terminals, `space:N`
  handling, HTML label sizing, and shape-specific geometry.
- Requirement/GitGraph/Timeline/Treemap/Sequence/Sankey/C4/Journey/Pie/Radar/XYChart/Gantt: move repeated text constants
  into generated overrides and close the remaining text-geometry, viewport, and font-size precedence gaps that affected
  parity fixtures.
- Theme/CSS: stop implicitly applying `base` defaults under `theme=default`, seed Mermaid-like base/neutral xychart
  defaults, and prefer `themeVariables.fontFamily` in emitted root SVG styles.
- Core/Layout internals: clean the remaining strict Clippy offenders in `dugong-graphlib`, `dugong`, and parser helper
  code, and scope vendored `manatee` FCoSE lint exceptions to the algorithm module so current stable Clippy stays
  actionable outside the imported numeric code.
- Toolchain/CI: pin the workspace Rust toolchain to `1.87.0` and make CI install the same version explicitly, so
  release and local checks stop drifting with floating `stable`.
- Toolchain/CI: drop GitHub Actions `cargo fmt` / `cargo clippy` steps for now so release CI focuses on build, tests,
  and parity checks while the remaining render hot spots are still being aligned.
- Maintenance: normalize `rustfmt` output in parity/text/timeline/xtask helpers so the pinned toolchain now passes
  workspace format checks without local-vs-CI drift.

## [0.3.0] - 2026-03-02

### Added

- Promoted additional in-scope deferred fixtures into the committed corpus (state parser specs, flowchart icon specs,
  class diagram specs, and math examples) and generated upstream SVG baselines.

### Fixed

- Architecture: refresh compound bounds after FCoSE spring iterations before applying `relocateComponent`-style centering
  (fixes `parity-root` root `max-width` drift in deep compound/group fixtures).
- Flowchart: unescape quoted string labels (e.g. Windows paths like `C:\\Temp\\...`) and preserve Unicode punctuation in
  label text.
- `xtask compare-flowchart-svgs`: skip ELK flowchart fixtures requested via `layout: elk` / `flowchart.defaultRenderer=elk`
  (prevents layout failures while ELK parity is deferred).
- Flowchart: align icon node shape rendering with upstream Mermaid (`icon` vs `iconSquare`) to avoid NaN path data and
  restore SVG DOM parity for AWS icon fixtures.
- Flowchart: improved `iconSquare` RoughJS path parity (rounded-rect path structure) for upstream icon shape fixtures.
- Class: align `htmlLabels` split semantics more closely with Mermaid: notes now respect global `htmlLabels` + class padding, while relation title labels switch to SVG `<text>/<tspan>` + background groups only when `flowchart.htmlLabels=false` is explicitly active.
- Class: render `htmlLabels: false` labels via SVG `<text>/<tspan>` (avoid `<foreignObject>` DOM mismatches in parity
  baselines).
- Text: closer-to-upstream Mermaid Markdown tokenization for flowchart SVG labels and layout measurement (fixes
  underscore/emphasis boundary edge cases).
- Radar: fixed detailed-entry parsing so decimal values like `3.2` are not misparsed as axis `3` with value `0.2`.
- Treemap: tightened header parsing to match Mermaid CLI (`treemap:` / `treemap utilities` now fail) and preserved the
  upstream behavior where trailing whitespace-only lines are treated as a syntax error.
- `xtask audit-gaps`: avoid trimming trailing whitespace when parsing deferred fixtures (prevents false “parse OK” on
  grammars like Treemap that treat trailing whitespace-only lines as an error).
- `xtask audit-gaps`: added `--check-upstream-render-deferred-ok` to identify promotable deferred fixtures
  (in-scope + upstream render OK).
- `xtask` SVG DOM compares: further reduced noisy `parity-root` root viewport diffs by snapping `max-width`/`viewBox`
  to a coarser lattice (0.25px).
- `xtask gen-upstream-svgs` / `compare-state-svgs`: allow generating/validating upstream baselines for renderable state
  parser fixtures while skipping the known upstream-crashing `upstream_state_parser_spec` fixture.
- Architecture: improved compound/nesting layout alignment by extending the FCoSE port with a compound graph model and
  closer-to-upstream bounds/centroid propagation behavior.
- Architecture: improved edge parsing/modeling compatibility (including `lhsInto`/`rhsInto` metadata when present).
- Architecture: removed fixture-id keyed label wrapping/formatting special-cases by tightening `createText(...)`-like
  SVG label wrapping and matching Mermaid CLI attribute newline serialization (`&#10;`).
- `xtask` SVG DOM compares: stabilized anonymous edge wrapper ordering for Architecture and reduced non-actionable text
  diffs caused by line wrapping sensitivity.
- README: fixed the Stress gallery Architecture fixture reference and refreshed the Architecture showcase render.

### Not Released / WIP

- Architecture: geometry-level parity (placements, viewport, and routing coordinates) is still being aligned to upstream
  Cytoscape/FCoSE. SVG DOM parity is compared in `dom-mode parity`, so expect occasional layout snapshot churn while we
  tighten numeric fidelity.
- Flowchart: HTML-label `$$...$$` (KaTeX) fixtures now participate in strict DOM parity via the opt-in
  `NodeKatexMathRenderer`; only environments without the local `tools/mermaid-cli` toolchain still fall back to
  non-math comparisons.
- Flowchart: `flowchart-elk` layout is not implemented yet; compare tooling skips those fixtures (still kept in the
  corpus for parser coverage).
- `merman-core`: dropped support for legacy Architecture edge shorthand (e.g. `a L--R b`, `a (L--R) b`) to align with
  Mermaid@11.12.3's Langium parser; use port-colon syntax instead (e.g. `a:L -- R:b`).
- `merman-render`: introduced a pluggable `MathRenderer` interface for `$$...$$` math labels (no default KaTeX backend;
  pure-Rust remains the default).
- `xtask`: added `audit-gaps` to summarize parser-only fixtures and deferred corpus status (helps drive “missing
  implementation” work off reproducible reports).
- `xtask audit-gaps`: optionally probe upstream renderability for parser-only fixtures via Mermaid CLI (flags:
  `--check-upstream-render`, `--upstream-timeout-secs`).

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
- `import-upstream-html`: flowchart fixtures containing `$$...$$` math labels now use the stable `*_katex` suffix and
  participate in full SVG DOM parity when the local KaTeX backend is available.
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
