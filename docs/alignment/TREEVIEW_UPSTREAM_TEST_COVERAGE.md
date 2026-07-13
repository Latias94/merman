# TreeView Upstream Test Coverage (Mermaid@11.16.0)

Scope: Mermaid tag `@11.16.0`.

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

- `fixtures/treeView/upstream_docs_treeview_basic.mmd`
  - source: `repo-ref/mermaid/docs/syntax/treeView.md`
  - semantic snapshot: `fixtures/treeView/upstream_docs_treeview_basic.golden.json`
  - layout snapshot: `fixtures/treeView/upstream_docs_treeview_basic.layout.golden.json`
- Cypress rendering fixtures from `repo-ref/mermaid/cypress/integration/rendering/treeView/treeView.spec.ts`:
  - `fixtures/treeView/upstream_cypress_treeview_spec_should_render_a_simple_treeview_diagram_001.mmd`
  - `fixtures/treeView/upstream_cypress_treeview_spec_should_render_a_complex_treeview_diagram_002.mmd`
  - `fixtures/treeView/upstream_cypress_treeview_spec_should_render_a_complex_treeview_diagram_with_multiple_roots_003.mmd`
  - `fixtures/treeView/upstream_cypress_treeview_spec_should_render_a_treeview_diagram_with_custom_config_004.mmd`
- Parser-source accessibility fixture from
  `repo-ref/mermaid/packages/parser/tests/treeView.test.ts`:
  - `fixtures/treeView/upstream_parser_treeview_title_accessibility_spec.mmd`
  - semantic snapshot:
    `fixtures/treeView/upstream_parser_treeview_title_accessibility_spec.golden.json`
  - layout snapshot:
    `fixtures/treeView/upstream_parser_treeview_title_accessibility_spec.layout.golden.json`

## Upstream SVG Baselines

- `fixtures/upstream-svgs/treeView/upstream_docs_treeview_basic.svg`
- `fixtures/upstream-svgs/treeView/upstream_cypress_treeview_spec_should_render_a_simple_treeview_diagram_001.svg`
- `fixtures/upstream-svgs/treeView/upstream_cypress_treeview_spec_should_render_a_complex_treeview_diagram_002.svg`
- `fixtures/upstream-svgs/treeView/upstream_cypress_treeview_spec_should_render_a_complex_treeview_diagram_with_multiple_roots_003.svg`
- `fixtures/upstream-svgs/treeView/upstream_cypress_treeview_spec_should_render_a_treeview_diagram_with_custom_config_004.svg`
- `fixtures/upstream-svgs/treeView/upstream_parser_treeview_title_accessibility_spec.svg`

## Compare Coverage

- Family-local command: `cargo run -p xtask -- compare-tree-view-svgs`
- Upstream baseline reproducibility: `cargo run -p xtask -- check-upstream-svgs --diagram treeView --check-dom --dom-mode parity --dom-decimals 3`
- Current DOM gate: `compare-tree-view-svgs --check-dom --dom-mode parity --dom-decimals 3`
  passes for the committed baseline corpus.

## Root Viewport Residuals

Command run on 2026-06-04:

- `cargo run -p xtask -- compare-tree-view-svgs --check-dom --dom-mode parity-root --dom-decimals 3`

Result:

- `parity-root` is not green for the current corpus.
- Root `width` is aligned: upstream and local emit `width="100%"` for all committed treeView
  fixtures.
- Root `height` has no current attr residual: no committed treeView fixture emits a root `height`
  attr because the current corpus uses `useMaxWidth=true`.
- Residuals are concentrated in root `viewBox` width/height and the derived `style max-width`.

Representative raw root values:

| Fixture | Upstream `viewBox` w x h | Local `viewBox` w x h | Upstream `max-width` | Local `max-width` |
|---|---:|---:|---:|---:|
| `upstream_docs_treeview_basic` | `103.390625 x 145` | `99 x 138` | `103.391px` | `99px` |
| `upstream_cypress_treeview_spec_should_render_a_simple_treeview_diagram_001` | `76.015625 x 58` | `76.5390625 x 55.2` | `76.0156px` | `76.5390625px` |
| `upstream_parser_treeview_title_accessibility_spec` | `76.515625 x 87` | `76.5859375 x 82.8` | `76.5156px` | `76.5859375px` |

Classification:

- The treeView renderer derives `viewBox` and `max-width` directly from label `getBBox()`
  measurements.
- Local output uses headless text metrics, so root viewport parity remains a bounded
  browser-text-measurement residual.
- Subtree DOM parity remains the current admission signal for treeView; do not add broad
  fixture-specific root magic numbers for this family without a source-backed text metric fix.

## Not Yet Covered

- Exact Langium diagnostics and offsets.
- Full strict DOM parity for the current Cypress image snapshot corpus.
