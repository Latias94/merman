# Upstream SVG Baselines

This document describes how to generate **upstream Mermaid SVG outputs** that act as baselines for
1:1 parity work.

Baseline version: Mermaid `@11.12.2`.

## Why This Exists

Without upstream SVG baselines, it is easy to "visually align by feel" and regress output
in subtle ways (marker ids, viewBox sizing, CSS selectors, etc). Baselines make changes auditable.

## Golden Layers

To make 1:1 parity work tractable, `merman` keeps multiple kinds of goldens:

- Upstream SVG baselines (this doc): the authoritative end-to-end output from Mermaid (via CLI).
- Semantic snapshots: parser output snapshots for `fixtures/**/*.mmd` (generated via
  `cargo run -p xtask -- update-snapshots`).
- Layout golden snapshots: geometry-level snapshots (`*.layout.golden.json`) that validate the
  headless layout model and help localize diffs to layout vs. SVG rendering (see
  `docs/adr/0047-layout-golden-snapshots.md`).

## Recommended Additional Goldens

If we need tighter 1:1 parity coverage beyond ER, extend the golden strategy in these directions:

- **Diagram-by-diagram SVG compare reports** (like `compare-er-svgs`), including:
  - viewBox + width/height deltas
  - marker and defs checks (arrowheads, gradients, filters)
  - optional geometry probes (e.g. parse `<path d>` and compare command sequences at a high level)
- **Error/diagnostics snapshots** for known-invalid inputs (parse errors and runtime render errors),
  including line/column ranges and message text.
- **Security-level snapshots** for sanitization behavior (e.g. `securityLevel` differences, HTML
  label allowlists), to prevent accidental loosening.
- **Theme/style snapshots** that lock the generated CSS blocks for a small set of themes and config
  overrides (prevents silent selector drift).

## Tooling

We use `@mermaid-js/mermaid-cli` pinned under `tools/mermaid-cli/`.
The CLI version and Mermaid version do not always match 1:1, so we use `npm overrides`
to force Mermaid `11.12.2`.

Install:

- `cd tools/mermaid-cli && npm install`

## Generate (ER only)

- `cargo run -p xtask -- gen-upstream-svgs --diagram er`

Outputs to:

- `fixtures/upstream-svgs/er/*.svg`

## Generate (Sequence)

- `cargo run -p xtask -- gen-upstream-svgs --diagram sequence`

Outputs to:

- `fixtures/upstream-svgs/sequence/*.svg`

## Generate (Info)

- `cargo run -p xtask -- gen-upstream-svgs --diagram info`

Outputs to:

- `fixtures/upstream-svgs/info/*.svg`

## Generate (Pie)

- `cargo run -p xtask -- gen-upstream-svgs --diagram pie`

Outputs to:

- `fixtures/upstream-svgs/pie/*.svg`

## Generate (Sankey)

- `cargo run -p xtask -- gen-upstream-svgs --diagram sankey`

Outputs to:

- `fixtures/upstream-svgs/sankey/*.svg`

## Generate (Packet)

- `cargo run -p xtask -- gen-upstream-svgs --diagram packet`

Outputs to:

- `fixtures/upstream-svgs/packet/*.svg`

## Generate (Timeline)

- `cargo run -p xtask -- gen-upstream-svgs --diagram timeline`

Outputs to:

- `fixtures/upstream-svgs/timeline/*.svg`

## Generate (Journey)

- `cargo run -p xtask -- gen-upstream-svgs --diagram journey`

Outputs to:

- `fixtures/upstream-svgs/journey/*.svg`

## Generate (Kanban)

- `cargo run -p xtask -- gen-upstream-svgs --diagram kanban`

Outputs to:

- `fixtures/upstream-svgs/kanban/*.svg`

## Generate (Gantt)

- `cargo run -p xtask -- gen-upstream-svgs --diagram gantt`

Outputs to:

- `fixtures/upstream-svgs/gantt/*.svg`

## Generate (GitGraph)

- `cargo run -p xtask -- gen-upstream-svgs --diagram gitgraph`

Outputs to:

- `fixtures/upstream-svgs/gitgraph/*.svg`

## Generate (C4)

- `cargo run -p xtask -- gen-upstream-svgs --diagram c4`

Outputs to:

- `fixtures/upstream-svgs/c4/*.svg`

Notes:

- Mermaid C4 has known render-time type assumptions that make some valid parser fixtures
  non-renderable (e.g. kv-objects stored in `label.text`, or `UpdateElementStyle(..., techn="Rust")`
  storing `techn` as a raw string).
- `xtask gen-upstream-svgs --diagram c4` skips such fixtures when generating baselines.

## Generate (Block)

- `cargo run -p xtask -- gen-upstream-svgs --diagram block`

Outputs to:

- `fixtures/upstream-svgs/block/*.svg`

## Generate (Radar)

- `cargo run -p xtask -- gen-upstream-svgs --diagram radar`

Outputs to:

- `fixtures/upstream-svgs/radar/*.svg`

## Generate (Treemap)

- `cargo run -p xtask -- gen-upstream-svgs --diagram treemap`

Outputs to:

- `fixtures/upstream-svgs/treemap/*.svg`

## Parser-Only Fixtures

Some fixtures are intentionally **parser-only** (they validate semantic parsing but are not
renderable in upstream Mermaid at the pinned version).

Convention:

- Any fixture whose filename contains `_parser_only_` (or `_parser_only_spec`) is skipped by:
  - `xtask gen-upstream-svgs`
  - `xtask check-upstream-svgs`
  - diagram compare tasks like `xtask compare-flowchart-svgs`

## Generate (C4 Stage B)

Generate local Stage-B C4 SVG outputs (not upstream baselines):

- `cargo run -p xtask -- gen-c4-svgs`

Outputs to:

- `target/svgs/c4/*.svg`

## Compare (C4)

Generate a report comparing upstream C4 SVGs and the current Rust Stage-B C4 output:

- `cargo run -p xtask -- compare-c4-svgs --check-dom --dom-mode parity --dom-decimals 3`

Notes:

- Mermaid derives C4 type-line `textLength` values from browser font metrics
  (`calculateTextWidth` + `getBBox`). To make DOM parity reproducible in a headless Rust context,
  `merman-render` vendors the observed `textLength` values for built-in C4 shape types
  at Mermaid `11.12.2` (generated file: `crates/merman-render/src/generated/c4_type_textlength_11_12_2.rs`).
- Regenerate the table from upstream baselines:
  - `cargo run -p xtask -- gen-c4-textlength`

## Generate (All supported diagrams)

- `cargo run -p xtask -- gen-upstream-svgs --diagram all`

## Verify Baselines (All supported diagrams)

Regenerate upstream SVGs into `target/upstream-svgs-check/` and verify they match the pinned
baselines under `fixtures/upstream-svgs/`:

- `cargo run -p xtask -- check-upstream-svgs --diagram all`

Notes:

- Most diagrams are compared as **raw SVG bytes** (exact string match).
- `state` diagrams are compared using a **structure-level DOM signature** by default because the
  upstream Mermaid renderer uses rough/stochastic geometry output (not byte-stable). The DOM check
  ignores `<path d>` / `data-points` payloads and normalizes generated ids.
- `gitGraph` diagrams are compared using a **structure-level DOM signature** by default because the
  upstream Mermaid parser auto-generates commit ids with random suffixes (not byte-stable).
- `gantt` diagrams are compared using a **structure-level DOM signature** by default because output
  can depend on the rendering environment (page width via `parentElement.offsetWidth`) and may
  include a `today` marker whose x-position depends on the current date.
- To force DOM comparison for all diagrams (useful when iterating on tooling):
  - `cargo run -p xtask -- check-upstream-svgs --diagram all --check-dom --dom-mode structure --dom-decimals 3`

## Compare (ER)

Generate a small report comparing upstream SVGs and the current Rust Stage-B ER SVG output:

- `cargo run -p xtask -- compare-er-svgs`
- Fail the command if marker definitions diverge:
  - `cargo run -p xtask -- compare-er-svgs --check-markers`
- Fail the command if the **SVG DOM** diverges (ignores attribute order/whitespace and rounds
  numeric tokens for comparison):
  - `cargo run -p xtask -- compare-er-svgs --check-dom --dom-decimals 3` (default `--dom-mode parity`)
  - Use a looser, structure-only mode while iterating on DOM shape (replaces numeric tokens with
    `<n>`, ignores `data-points`, and ignores `<style>` text):
    - `cargo run -p xtask -- compare-er-svgs --check-dom --dom-mode structure --dom-decimals 3`
  - Use a parity-focused mode to ignore geometry noise (replaces numeric tokens in geometry attrs
    with `<n>`, ignores `data-points` and `<style>` text, and ignores `max-width` heuristics inside
    HTML label `<div>` style attributes):
    - `cargo run -p xtask -- compare-er-svgs --check-dom --dom-mode parity --dom-decimals 3`
  - For size/viewBox parity work, use `parity-root` which is identical to `parity` but also compares
    the root `<svg>` `viewBox` and `style` attributes:
    - `cargo run -p xtask -- compare-er-svgs --check-dom --dom-mode parity-root --dom-decimals 3`

## Generate (Flowchart Stage B)

Generate local Stage-B flowchart SVG outputs (not upstream baselines):

- `cargo run -p xtask -- gen-flowchart-svgs`

Outputs to:

- `target/svgs/flowchart/*.svg`

## Compare (Flowchart)

Generate a report comparing upstream flowchart SVGs and the current Rust Stage-B flowchart output:

- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3`
- Use the looser, structure-only mode while iterating on large layout/routing refactors:
  - `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode structure --dom-decimals 3`
- For root `<svg>` viewport parity (`viewBox` / `style="max-width: ..."`), use `parity-root` and the root-delta report:
  - `cargo run -p xtask -- compare-flowchart-svgs --dom-mode parity-root --dom-decimals 3 --report-root`
  - See `docs/alignment/FLOWCHART_ROOT_VIEWBOX_PARITY_GAPS.md` for current status.

## Compare (Block)

Generate a report comparing upstream block SVGs and the current Rust Stage-B block output:

- `cargo run -p xtask -- compare-block-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Compare (Radar)

Generate a report comparing upstream radar SVGs and the current Rust Stage-B radar output:

- `cargo run -p xtask -- compare-radar-svgs --check-dom --dom-mode parity --dom-decimals 3`

Notes:

- Flowchart `domId` suffixes depend on FlowDB `vertexCounter` (Jison `addVertex(...)` call order, including `@{...}` shapeData passes).
  The flowchart semantic model includes `vertexCalls` to make this deterministic and reproducible in Rust.

## Generate (StateDiagram Stage B)

Generate local Stage-B stateDiagram SVG outputs (not upstream baselines):

- `cargo run -p xtask -- gen-state-svgs`

Outputs to:

- `target/svgs/state/*.svg`

## Generate (ClassDiagram Stage B)

Generate local Stage-B classDiagram SVG outputs (not upstream baselines):

- `cargo run -p xtask -- gen-class-svgs`

Outputs to:

- `target/svgs/class/*.svg`

Notes:

- Stage-B class layout sizes nodes using the global Mermaid `fontSize` and a fixed `line-height: 1.5`
  (matching upstream HTML label structure) to keep layout and SVG rendering consistent and avoid
  label overlap.

## Compare (StateDiagram)

Generate a report comparing upstream stateDiagram SVGs and the current Rust Stage-B stateDiagram
output (DOM signature comparison; upstream is not byte-stable):

- `cargo run -p xtask -- compare-state-svgs --dom-mode structure --dom-decimals 3`

## Compare (ClassDiagram)

Generate a report comparing upstream classDiagram SVGs and the current Rust Stage-B classDiagram
output (DOM signature comparison):

- `cargo run -p xtask -- compare-class-svgs --dom-mode parity --dom-decimals 3`
- Use the looser, structure-only mode while iterating on DOM shape:
  - `cargo run -p xtask -- compare-class-svgs --dom-mode structure --dom-decimals 3`

Notes:

- `fixtures/class/upstream_text_label_variants_spec.mmd` is excluded (Mermaid CLI failure at 11.12.2).
- `fixtures/class/upstream_parser_class_spec.mmd` is excluded because the upstream SVG contains
  prototype-key rendering artifacts (nested `g.root` / `translate(NaN, ...)`), while `merman`
  renders deterministically.

Notes:

- The flowchart DOM compare is intentionally looser than ER while Stage-B rendering is still being
  brought up. It ignores `<path d>` and `data-points` geometry payloads and normalizes child order
  for container groups like `g.root` by using the first descendant cluster id as a sort hint.

## Compare (Info)

Generate a report comparing upstream info SVGs and the current Rust Stage-B info output:

- `cargo run -p xtask -- compare-info-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Compare (Pie)

Generate a report comparing upstream pie SVGs and the current Rust Stage-B pie output:

- `cargo run -p xtask -- compare-pie-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Compare (Packet)

Generate a report comparing upstream packet SVGs and the current Rust Stage-B packet output:

- `cargo run -p xtask -- compare-packet-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Compare (Timeline)

Generate a report comparing upstream timeline SVGs and the current Rust Stage-B timeline output:

- `cargo run -p xtask -- compare-timeline-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Compare (Journey)

Generate a report comparing upstream journey SVGs and the current Rust Stage-B journey output:

- `cargo run -p xtask -- compare-journey-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Compare (Kanban)

Generate a report comparing upstream kanban SVGs and the current Rust Stage-B kanban output:

- `cargo run -p xtask -- compare-kanban-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Compare (GitGraph)

Generate a report comparing upstream gitGraph SVGs and the current Rust Stage-B gitGraph output:

- `cargo run -p xtask -- compare-gitgraph-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Notes

- The generator passes `--svgId <fixture_stem>` to make the root SVG id deterministic.
- If rendering fails for a fixture, the tool still writes as many SVGs as possible and records
  failures to `fixtures/upstream-svgs/<diagram>/_failures.txt` (the command will exit non-zero).
- We currently store raw upstream SVG outputs. For `state` diagrams, upstream output is not
  byte-stable, so baseline verification uses a structure-level DOM signature instead of a raw byte
  compare.
- `gitgraph` output is not byte-stable because commit ids can be randomly generated by upstream
  Mermaid when not explicitly specified. Baseline verification uses a structure-level DOM signature
  by default.

## Known Upstream Rendering Failures / Anomalies (as of Mermaid 11.12.2)

- `fixtures/state/upstream_state_parser_spec.mmd`: includes `__proto__`/`constructor` states; Mermaid CLI currently crashes (excluded from `gen-upstream-svgs` / `check-upstream-svgs`).
- `fixtures/class/upstream_text_label_variants_spec.mmd`: includes a whitespace-only label (`" "`); Mermaid CLI currently fails (NaN transforms / missing SVG in render tree; excluded from `gen-upstream-svgs` / `check-upstream-svgs`).
- `fixtures/class/upstream_parser_class_spec.mmd`: includes `__proto__`/`constructor` classes; Mermaid CLI renders but produces invalid transforms (NaN) and duplicated root groups (excluded from `compare-class-svgs`).
- `fixtures/gantt/today_marker_and_axis.mmd`: Mermaid CLI crashes while parsing `topAxis` (`yy.TopAxis is not a function`) (excluded from `gen-upstream-svgs` / `check-upstream-svgs`).
- `fixtures/gantt/click_loose.mmd` / `fixtures/gantt/click_strict.mmd`: contain non-canonical `click ... href "<url>" "<extra>"` syntax that Mermaid CLI rejects (excluded from `gen-upstream-svgs` / `check-upstream-svgs`).
- `fixtures/gantt/dateformat_hash_comment_truncates.mmd` / `fixtures/gantt/excludes_hash_comment_truncates.mmd`: rely on `#` inline comment truncation that Mermaid CLI rejects (excluded from `gen-upstream-svgs` / `check-upstream-svgs`).

These exclusions keep baseline verification and compare reports actionable for the rest of the suite.
