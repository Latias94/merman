# Changelog

All notable changes to this project will be documented in this file.

The format is based on *Keep a Changelog*, and this project adheres to *Semantic Versioning*.

## [Unreleased]

### Added

- `xtask`: extended `gen-upstream-svgs` and `compare-svg-xml` to support generating/comparing SVG baselines from custom
  fixture roots (useful for strict XML diffs when iterating on layout parity).
- Docs: expanded `docs/workstreams/*` guidance for text-measurement parity work (including `parity-root` root viewport checks).
- Flowchart: added the upstream Cypress fixture `upstream_cypress_flowchart_v2_spec_should_be_possible_to_use_syntax_to_add_labels_with_trail_spaces_067` (trail spaces + edge/link), including upstream SVG baselines.
- Flowchart: added a stress fixture for HTML label wrapping with a URL-heavy token under `wrappingWidth=200`.
- Flowchart: added a stress fixture for HTML label whitespace handling (`&nbsp;`, multiple spaces, trailing spaces).
- Flowchart: added a stress fixture for `htmlLabels: true` default-class/default-node styling semantics (`classDef default` + `style default`).
- Flowchart: added a stress fixture for `htmlLabels: true` Markdown labels that mix paragraphs with raw/list-style lines.
- State/Requirement: added stress fixtures for HTML-label Markdown that keeps `<br/>- ...` list-like continuations inside the same paragraph.
- Class: added a stress fixture for HTML-label edge Markdown that keeps `<br/>- ...` list-like continuations inside the same paragraph.
- Class/Mindmap: added stress fixtures for HTML-label font-size inheritance quirks (Mermaid CLI / Puppeteer), including upstream SVG baselines.
- Class: added a stress fixture for SVG-label wrapping when `fontSize` differs from `themeVariables.fontSize` (including upstream SVG baseline).
- State/Sequence/Gantt/Journey/ER/Requirement/Block/Radar/Kanban/GitGraph/Treemap: added stress fixtures for font-size precedence (`themeVariables.fontSize: "NNpx"` vs `fontSize: N`),
  including upstream SVG baselines + local layout goldens.
- Timeline: added a stress fixture for unknown XML entity escaping (including upstream SVG baseline).
- Timeline: added a stress fixture for `themeVariables.fontSize` precedence over top-level `fontSize` (including upstream SVG baseline + local layout goldens).
- Flowchart/State: added stress fixtures for `classDef`/`style` text overrides (font-family/font-size/opacity),
  including upstream SVG baselines.
- Architecture: added a stress fixture for `iconText` HTML that wraps inline code in a root-level anchor inside `foreignObject`,
  including upstream SVG baseline + local model/layout goldens.

### Fixed

- Flowchart: decode Mermaid entity placeholders in subgraph titles (contributed by @aydiler in PR #1:
  https://github.com/Latias94/merman/pull/1).
- Render: decode Mermaid `encodeEntities(...)` placeholders in SVG label text across diagrams (prevents raw `ﬂ°…¶ß`
  sequences from leaking into output).
- Flowchart: treat `@{...}` node declarations as subgraph members even when the subgraph contains no internal edges
  (restores upstream-style cluster membership / SVG DOM structure).
- Mindmap: decode Mermaid entity placeholders after Markdown sanitization while preserving valid XML entities (prevents malformed `&...;` sequences in SVG output).
- Sequence: prefer the global `fontSize` over `sequence.messageFontSize` when emitting SVG text styles (aligns with Mermaid CLI baselines).
- Treemap: align the leaf label font sizing for `Item A1` with upstream Mermaid CLI baselines (prevents a 1px shrink
  due to text measurement differences).
- Class/Mindmap: match Mermaid CLI baselines by measuring HTML `<foreignObject>` labels at the browser default (16px)
  instead of relying on SVG-root `font-size` inheritance when `themeVariables.fontSize` is overridden.
- Class: match upstream Mermaid SVG-label wrapping when `fontSize` (used by `calculateTextWidth`) differs from the root
  `font-size` inherited by `<text>` (often from `themeVariables.fontSize`).
- Text: treat backtick-delimited spans as literal during Mermaid Markdown tokenization so emphasis/strong delimiters
  inside them are not interpreted (aligns with upstream Mermaid CLI baselines for inline-code-like labels).
- `xtask` SVG DOM compares: include inline `style` `font-size` for `<text>/<tspan>` nodes in `dom-mode parity` (catch
  text sizing drift without comparing full style strings).
- Flowchart: honor implicit `classDef default` styling for unlabeled/default-class nodes under `htmlLabels: true`, while still layering node-id `style default ...` overrides for a node literally named `default`.
- Flowchart/Text: keep Mermaid HTML-label Markdown block semantics when a label mixes a normal paragraph with raw/list-style lines (emit `<p>...</p>` plus collapsed literal block text instead of turning everything into `<br/>`-separated paragraphs).
- Flowchart/Core+Render: keep bare-backtick pipe edge labels literal instead of upgrading them to Markdown, and mirror Mermaid SVG-label behavior where backtick-wrapped `text` edge labels collapse to the empty placeholder while HTML-label mode still preserves the literal backticks/raw tags.
- Flowchart/Text: align strict-XML metrics for literal-backtick pipe edge probes across both `htmlLabels` paths, including the common SVG `Start`/`End` bbox lattice used by those fixtures.
- Flowchart/Text+Render: align quoted Markdown edge labels that mix closing `</br>` and raw inline HTML (`<strong>...</strong>`) with Mermaid across both `htmlLabels` paths: HTML-label mode now measures/renderers the generated XHTML fragment like browser DOM, while SVG-label mode keeps raw tags literal but wraps them onto Mermaid-matching `<tspan>` lines.
- State/Requirement/Text: preserve Mermaid HTML-label paragraph semantics for `<br/>- ...` continuation lines, and measure requirement multiline field rows with the same height/max-width behavior as upstream.
- Class/Text: route class HTML-label Markdown rendering through the shared XHTML helper so inline `<br/>` continuations render as Mermaid paragraphs instead of escaped literal tags.
- Text/Class: reinterpret malformed partial `**...*` HTML-label star runs the same way Mermaid/CommonMark does, so class members like `+inline: **bold**` (after classifier stripping) emit literal `*` + `<em>bold</em>` instead of fully literal text.
- Class/Text: size single-glyph SVG class titles from Mermaid-style bold computed text length (instead of the generic SVG bbox path), removing the remaining `htmlLabels=false` simple-node/root-viewport drift on `probe_class_htmllabels_false_981`.
- ER/Text: route ER relationship HTML labels through Mermaid-style Markdown rendering and markdown-aware measurement, so edge labels honor emphasis (`**...**`, `_..._`) and existing `<br/>` line-break fixtures keep upstream spacing.
- ER/Text: preserve inline-code backticks in ER HTML labels so entity/attribute labels keep literal `` `**...**` `` text instead of emitting synthetic `<code>` / `<strong>` DOM.
- Mindmap/Text: route complex markdown HTML labels through Mermaid-style XHTML fragments for DOM output and measurement, so mixed paragraph + list/raw-block labels collapse like upstream instead of emitting synthetic `<ul><li>...` DOM.
- Architecture/Text: normalize `iconText` HTML fragments with Mermaid/Chromium's SVG-namespace `foreignObject` parsing semantics, so root-level `<a>` wrappers no longer retain inline HTML descendants that upstream breaks into sibling nodes.
- Architecture: align singleton top-level `iconText` service Y offset and root `viewBox` with Mermaid, removing the remaining strict-XML drift on anchor/html probe fixtures.
- Flowchart: align HTML label wrapping and Markdown handling with upstream Mermaid:
  - node HTML label `max-width` respects `flowchart.wrappingWidth` (edge labels remain capped at 200px),
  - blank-line (`\\n\\n`) breaks are emitted as paragraph splits (`</p><p>`) instead of `<br /><br />`,
  - underscore-heavy identifiers (e.g. `a__node`) no longer get misparsed as emphasis.
- Flowchart: align SVG edge label background rectangle offset (`y=-1`) with upstream Mermaid.
- Flowchart: match Mermaid's flowchart font sizing rules by reading `themeVariables.fontSize` only (top-level `fontSize`
  no longer affects flowchart layout/label measurement).
- State: align state label font sizing by preferring `themeVariables.fontSize` (including `"NNpx"` strings) over the
  legacy top-level `fontSize` when computing text layout/measurement.
- ER: align entity/root font sizing with `themeVariables.fontSize` (including `"NNpx"` strings) while keeping
  relationship-label measurement at Mermaid's fixed 14px.
- Kanban: align card/section layout font sizing with `themeVariables.fontSize` (including `"NNpx"` strings)
  and pin the new smoke fixture's `parity-root` viewport to upstream.
- GitGraph: align branch-label layout font sizing with `themeVariables.fontSize` (including `"NNpx"` strings)
  and pin the new smoke fixture's `parity-root` viewport to upstream.
- Block: align block node/edge layout font sizing with `themeVariables.fontSize` (including `"NNpx"` strings)
  and pin the new smoke fixture's `parity-root` viewport to upstream.
- Requirement: align diagram/root font sizing with `themeVariables.fontSize` (including `"NNpx"` strings),
  accept CSS-style `fontSize` values during layout/parity measurement, and pin the new smoke fixture's `parity-root` viewport to upstream.
- Flowchart/Class/GitGraph: pin the remaining `parity-root` root viewport overrides for text-style/font-size smoke fixtures
  and `upstream_merges_spec`, keeping the full `--dom-mode parity-root --dom-decimals 6` gate green.
- Text: model browser-like line-breaking inside punctuation-heavy tokens (URLs) for HTML label wrapping at max width.
- Text: align HTML label measured widths with upstream min-content expansion for long, hyphenated tokens (affects `foreignObject width="..."`).
- Text: avoid inflating flowchart HTML label height for quoted-string trailing-only whitespace (improves `parity-root` root viewport alignment).
- Text: align wrapped HTML label widths for inline-styled flowchart labels by basing width on wrapped layout (fixes large `parity-root` `max-width/viewBox` deltas in shape stress fixtures).
- Text: treat failed `__` delimiter runs as literal in Mermaid Markdown tokenization (fixes `a__b` being misparsed into emphasis spans).
- Theme: avoid implicitly applying `base` theme defaults when `theme=default` (fixes downstream color/style drift,
  notably in xychart).
- Theme: seed Mermaid `theme-base` / `theme-neutral` xychart defaults (background + plot palette) so `theme: base`
  renders match upstream Mermaid CLI SVG baselines.
- CSS: prefer `themeVariables.fontFamily` over legacy top-level `fontFamily` when emitting root SVG styles (aligns with Mermaid initialization semantics and upstream baselines).
- Timeline: align wrapping/height calculations and font-size parsing with upstream Mermaid CLI baselines:
  - support `themeVariables.fontSize` as a `"NNpx"` string where applicable,
  - replicate upstream `maxTaskHeight` quirk (`"[object Object]"` virtual label),
  - improve wrap stability for custom fonts without explicit generic fallbacks.

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
- Flowchart/Sequence: `$$...$$` (KaTeX) label DOM parity remains deferred; compare tooling skips these fixtures when
  `--check-dom` is enabled until a real `MathRenderer` backend exists.
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
- `import-upstream-html`: flowchart fixtures containing `$$...$$` math labels are imported as `*_parser_only_katex` (kept for parser/layout coverage, excluded from SVG DOM parity gates until KaTeX HTML label parity is implemented).
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
