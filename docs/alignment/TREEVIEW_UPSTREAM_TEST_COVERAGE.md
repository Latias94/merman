# TreeView Upstream Test Coverage (Mermaid@11.16.1)

Scope: Mermaid tag `@11.16.1`.

Phase 2 admission backlog: `docs/alignment/PHASE2_PARITY_BACKLOG.md`.

## Upstream Sources

- Parser tests: `repo-ref/mermaid/packages/parser/tests/treeView.test.ts`
- Rendering tests: `repo-ref/mermaid/cypress/integration/rendering/treeView/treeView.spec.ts`
- Syntax docs: `repo-ref/mermaid/docs/syntax/treeView.md`

## Covered Locally

- `should parse empty treeView`:
  - parser path covered by `crates/merman-core/src/diagrams/tree_view.rs`
- `should parse a treeView with only a root node`:
  - parser unit coverage in `crates/merman-core/src/diagrams/tree_view.rs`
- `should parse a treeView with multiple words within a node`:
  - quoted-string parser supports spaces in node names
- `should parse a treeView with child nodes`:
  - parser unit coverage in `crates/merman-core/src/diagrams/tree_view.rs`
- `should parse a treeView with title`
- `should parse a treeView with accTitle`
- `should parse a treeView with accDescr`
- `should parse a treeView with multiple accessibility attributes`
  - parser unit coverage in `crates/merman-core/src/diagrams/tree_view.rs`
  - SVG root accessibility DOM coverage in
    `fixtures/treeView/upstream_parser_treeview_title_accessibility_spec.mmd`
- Cypress custom config example:
  - `crates/merman-render/tests/tree_view_svg_test.rs`
- Mermaid 11.16 TreeView parser additions:
  - bare labels, dotfiles, file names with spaces, single/double quoted labels, and empty quoted
    labels are covered by `parses_mermaid_11_16_node_annotations_and_bare_names`
  - trailing slash directory detection strips the slash and emits `nodeType: "directory"`
  - `:::class`, `icon(...)`, `icon(none)`, empty `icon()`, and `## description` annotations are
    parsed into the typed render model and semantic JSON
  - box-drawing input (`├──`, `└──`, `│`, plus heavy variants through the shared scanner) is parsed
    as indentation-equivalent tree input; mixed indentation in box-drawing mode is rejected
  - editor facts preserve original-source spans for box-drawing node names and annotation payloads
- Mermaid 11.16 TreeView render/config additions:
  - `showIcons`, `defaultIconPack`, `filenameIcons`, and `extensionIcons` are read by the TreeView
    layout config and resolved with Mermaid's explicit-icon-first priority
  - SVG output includes `treeView-node-dir`, custom class propagation, `treeView-node-icon`,
    `treeView-node-description`, and `treeView-highlight-bg` DOM/CSS coverage
  - configured Iconify pack bodies render in deterministic 14-by-14 nested SVGs while preserving
    their source viewBox; repeated nodes share one symbol definition
  - internal Iconify IDs and references are scoped per diagram and TreeView symbol, with
    deterministic output across repeated renders
  - built-in `file`/`folder` bodies take precedence over registry entries, while missing packs or
    icons use Mermaid's standard 80-by-80 unknown-icon body at the same 14px display size
  - CLI coverage exercises the local Iconify JSON loader through `SvgRenderOptions` into TreeView;
    renderer code performs no filesystem, package-manager, or network access
  - theme roles `iconColor`, `descriptionColor`, `highlightBg`, and `highlightStroke` are covered by
    `PresentationTheme` tests; full config-pipeline admission for those newer theme fields remains
    tracked separately from TreeView parser/model parity

## Fixture Coverage

- The active corpus contains 17 fixtures:
  - 15 exact Mermaid Cypress render cases from
    `repo-ref/mermaid/cypress/integration/rendering/treeView/treeView.spec.ts`, covering quoted and
    bare labels, multiple roots, custom config, class annotations, descriptions, Iconify packs,
    filename/extension icon maps, default-pack resolution, unknown-icon fallback, hidden icons,
    Unicode/consecutive spaces, emoji icons, and combined annotations;
  - `fixtures/treeView/upstream_docs_treeview_basic.mmd` from the syntax documentation;
  - `fixtures/treeView/upstream_parser_treeview_title_accessibility_spec.mmd` from
    `repo-ref/mermaid/packages/parser/tests/treeView.test.ts`, retaining title/accessibility parser
    evidence in the snapshot lane.
- Every active fixture has semantic/layout evidence and a pinned Mermaid SVG. The full 15-case
  Cypress set is intentionally retained because the 11.16 icon and annotation behavior is not
  reducible to the older four-example subset.

## Upstream SVG Baselines

The 17 corresponding SVGs live under `fixtures/upstream-svgs/treeView/` with per-file input/output
hashes and generated provenance. The fixture names are generated from the pinned source calls; the
admission gate discovers them from the active directory rather than maintaining a second hand-written
list.

## Compare Coverage

- Family-local command: `cargo run -p xtask -- compare-tree-view-svgs`
- Upstream baseline reproducibility: `cargo run -p xtask -- check-upstream-svgs --diagram treeView --check-dom --dom-mode parity --dom-decimals 3`
- Current DOM gate: `compare-tree-view-svgs --check-dom --dom-mode parity --dom-decimals 3`
  passes for the committed baseline corpus.

## Verification

```text
cargo run -p xtask -- compare-tree-view-svgs --check-dom --dom-mode parity --dom-decimals 3
cargo run -p xtask -- check-upstream-svgs --diagram treeView --check-dom --dom-mode parity --dom-decimals 3
```

## Root Viewport Residuals

Command run on 2026-07-17:

- `cargo run -p xtask -- compare-tree-view-svgs --check-dom --dom-mode parity-root --dom-decimals 3`

Result:

- Normal `parity` is green for the current corpus. `parity-root` reports 13 root-only residuals;
  normalized descendants match in every case.
- Root `width` is aligned: upstream and local emit `width="100%"` for all committed treeView
  fixtures.
- Root `height` has no current attr residual: no committed treeView fixture emits a root `height`
  attr because the current corpus uses `useMaxWidth=true`.
- Residuals are concentrated in root `viewBox` width/height and the derived `style max-width`.

Representative raw root values:

| Fixture | Upstream `viewBox` w x h | Local `viewBox` w x h | Upstream `max-width` | Local `max-width` |
|---|---:|---:|---:|---:|
| `upstream_docs_treeview_basic` | `103.390625 x 145` | `103.40625 x 145` | `103.391px` | `103.406px` |
| `upstream_cypress_treeview_spec_should_render_a_simple_treeview_diagram_001` | `76.015625 x 58` | `76.03125 x 58` | `76.016px` | `76.031px` |
| `upstream_cypress_treeview_spec_should_preserve_consecutive_spaces_and_unicode_in_labels_013` | `170.59375 x 119` | `166.625 x 115.59375` | `170.594px` | `166.625px` |
| `upstream_cypress_treeview_spec_should_render_emoji_as_icons_with_the_default_icons_hidden_014` | `150.953125 x 157` | `145.96875 x 143.375` | `150.953px` | `145.969px` |

Classification:

- The treeView renderer derives `viewBox` and `max-width` directly from label `getBBox()`
  measurements.
- Eleven residuals are ASCII direct-text bbox quantization differences. Their signed width deltas
  range from one to two `1/64px` browser lattice steps, including both positive and negative
  directions. A uniform correction would therefore be wrong; exact convergence requires a
  reusable direct-text horizontal DOM profile rather than fixture or complete-label values.
- Two residuals contain emoji or other non-ASCII glyphs. The upstream macOS Chromium baseline
  resolves those glyphs through system fallback fonts, while the deterministic vendored profile
  deliberately has no OS-specific fallback-font table. Their width and row-height differences are
  therefore browser/system-font measurements, not TreeView layout semantics.
- `RawBBoxWidth` and `RawBBoxHeight` are already distinct render-environment operations. A host
  with the installed browser fonts can answer them exactly without a TreeView-specific ABI.
- These observations belong in the attributable browser root diagnostic artifact. Do not add
  fixture ids, complete labels, a blanket `1/64px` adjustment, or emoji-specific geometry to the
  production renderer.

## Not Yet Covered

- Exact Langium diagnostics and offsets.
- Full strict DOM parity for the current Cypress image snapshot corpus.
