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
| Sequence | yes | yes | Stage B (wip) + debug | yes | yes (`xtask compare-sequence-svgs`) |
| Others (gantt, …) | yes | no | no | no | no |

Note: Sequence SVG Stage B is in progress; `xtask compare-sequence-svgs` is expected to report
mismatches until the renderer reaches parity with upstream baselines.
Recent progress: sequence `alt`/`loop` frames derive separator placement from layout message y-coordinates;
the dashed separators are extended by a tiny epsilon to avoid visible end gaps when the dash period lands
exactly on the frame boundary.
Recent progress: sequence headless layout now models notes and `rect` blocks as layout nodes (`note-*`, `rect-*`),
so SVG viewBox/bounds can expand to match upstream baselines (e.g. left-of notes and nested rect blocks).
Recent progress: sequence headless layout now models self-messages with `startx == stopx` and adds the extra
vertical bump Mermaid applies for the loop curve; Stage B SVG renders self-messages as `<path>` and renders
participant types (`boundary`, `control`, `entity`, `database`, `collections`, `queue`) with Mermaid-like DOM
structure (the `participant_types` upstream baseline now matches in DOM parity mode).

## Alignment Sanity Checks

- Internal consistency: `cargo run -p xtask -- check-alignment`
  - ensures every fixture has a `.golden.json`
  - ensures coverage docs reference existing local paths
