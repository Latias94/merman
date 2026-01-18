# Alignment Status (Mermaid Parity Dashboard)

Baseline: Mermaid `@11.12.2` (see `repo-ref/REPOS.lock.json`).

This file is a lightweight dashboard of what is currently implemented and what is covered by
goldens/baselines. It is intentionally short and should stay true even as fixtures grow.

## Golden Layers

- Semantic snapshots (`fixtures/**/*.golden.json`):
  - Scope: parsing + semantic model output.
  - Validator: `cargo nextest run -p merman-core` (snapshot test) or full `cargo nextest run`.
  - Maintenance: `cargo run -p xtask -- update-snapshots`.
- Layout snapshots (`fixtures/**/*.layout.golden.json`):
  - Scope: geometry layer (nodes/edges/clusters/labels/bounds).
  - Validator: `cargo nextest run -p merman-render` (layout snapshot test) or full `cargo nextest run`.
  - Maintenance: `cargo run -p xtask -- update-layout-snapshots [--diagram <name>]`.
- Upstream SVG baselines (`fixtures/upstream-svgs/**`):
  - Scope: authoritative Mermaid end-to-end SVG output (generated via official CLI).
  - How-to: `docs/rendering/UPSTREAM_SVG_BASELINES.md`.

## Diagram Coverage Matrix

Legend:

- Parse: `Engine::parse_diagram` supports the diagram and is covered by semantic snapshots.
- Layout: `layout_parsed` supports the diagram and is covered by layout snapshots.
- Render: a Rust SVG renderer exists (may be “debug” stage vs. “parity” stage).
- Upstream SVG: upstream baselines are stored under `fixtures/upstream-svgs/<diagram>/`.
- Compare: an automated compare report exists against upstream baselines.

| Diagram | Parse | Layout | Render | Upstream SVG | Compare |
|---|---:|---:|---|---:|---:|
| ER | yes | yes | Stage B + debug | yes | yes (`xtask compare-er-svgs`) |
| Flowchart | yes | yes | Stage B + debug | yes | yes (`xtask compare-flowchart-svgs`) |
| State | yes | yes | Stage B + debug | yes | yes (`xtask compare-state-svgs`) |
| Class | yes | yes | Stage B + debug | yes | yes (`xtask compare-class-svgs`) |
| Sequence | yes | yes | Stage B + debug | yes | yes (`xtask compare-sequence-svgs`) |
| Info | yes | yes | Stage B | yes | yes (`xtask compare-info-svgs`) |
| Pie | yes | yes | Stage B | yes | yes (`xtask compare-pie-svgs`) |
| Others (gantt, …) | yes | no | no | no | no |

Recent progress: sequence `alt`/`loop` frames derive separator placement from layout message y-coordinates;
the dashed separators now use the exact same x-coordinates as the frame edges to match upstream SVG and
avoid sub-pixel gaps at the frame border.
Recent progress: sequence headless layout now models notes and `rect` blocks as layout nodes (`note-*`, `rect-*`),
so SVG viewBox/bounds can expand to match upstream baselines (e.g. left-of notes and nested rect blocks).
Recent progress: sequence headless layout now models self-messages with `startx == stopx` and adds the extra
vertical bump Mermaid applies for the loop curve; Stage B SVG renders self-messages as `<path>` and renders
participant types (`boundary`, `control`, `entity`, `database`, `collections`, `queue`) with Mermaid-like DOM
structure (the `participant_types` upstream baseline now matches in DOM parity mode).
Recent progress: sequence Stage B now renders `opt`/`par` blocks (including `par over`) and `box` frames;
empty block labels are rendered as a zero-width space (matching upstream SVG behavior).
Recent progress: sequence Stage B now treats HTML `<br>` variants as line breaks in participant labels, notes,
and message texts, matching upstream DOM structure in `html_br_variants_and_wrap`; empty message labels
(trailing colon) now still produce a message text node like upstream.
Recent progress: sequence Stage B now matches upstream SVG DOM for the current fixture set in parity mode
(`cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity --dom-decimals 3`).
Recent progress: `info` and `pie` Stage B parity renderers exist and are validated against upstream
SVG baselines via `xtask compare-info-svgs` / `xtask compare-pie-svgs`.

## Alignment Sanity Checks

- Internal consistency: `cargo run -p xtask -- check-alignment`
  - ensures every fixture has a `.golden.json`
  - ensures coverage docs reference existing local paths
