# merman

Mermaid, but headless, in Rust.

`merman` is a Rust, headless, 1:1 re-implementation of Mermaid pinned to `mermaid@11.12.2`.
The upstream Mermaid implementation is the spec (see `docs/adr/0014-upstream-parity-policy.md`).

## What you get

- Parse Mermaid into a semantic JSON model (headless)
- Compute headless layout (geometry + routes) as JSON
- Render SVG (parity-focused DOM)
- Render PNG (SVG rasterization via `resvg`)
- Render JPG (SVG rasterization via `resvg`)

Diagram coverage and current parity status live in `docs/alignment/STATUS.md`.

## Parity & goldens

This repo is built around reproducible alignment layers:

- Semantic snapshots: `fixtures/**/*.golden.json`
- Layout snapshots: `fixtures/**/*.layout.golden.json`
- Upstream SVG baselines: `fixtures/upstream-svgs/**`
- DOM parity gates: `xtask compare-all-svgs --check-dom` (see `docs/adr/0050-release-quality-gates.md`)

The goal is not “it looks similar”, but “it stays aligned”.

## CLI

- Detect diagram type:
  - `cargo run -p merman-cli -- detect path/to/diagram.mmd`
- Parse → semantic JSON:
  - `cargo run -p merman-cli -- parse path/to/diagram.mmd --pretty`
- Layout → layout JSON:
  - `cargo run -p merman-cli -- layout path/to/diagram.mmd --pretty`
- Render SVG:
  - `cargo run -p merman-cli -- render path/to/diagram.mmd --out out.svg`
- Render PNG:
  - Default output path (writes next to the input file):
    - `cargo run -p merman-cli -- render --format png path/to/diagram.mmd`
  - Explicit output path:
    - `cargo run -p merman-cli -- render --format png --out out.png path/to/diagram.mmd`
- Render JPG:
  - Default output path (writes next to the input file):
    - `cargo run -p merman-cli -- render --format jpg path/to/diagram.mmd`
  - Explicit output path:
    - `cargo run -p merman-cli -- render --format jpg --out out.jpg path/to/diagram.mmd`

## Library

- Headless parsing: `merman-core`
- Convenience API: `merman` (enable `render` for layout + SVG)
- Rendering + layout stack: `merman-render`
- Layout ports:
  - `dugong`: Dagre-compatible layout (port of `dagrejs/dagre`)
  - `dugong-graphlib`: graph container APIs (port of `dagrejs/graphlib`)
  - `manatee`: compound graph layouts (COSE/FCoSE ports)

## Development

- Format:
  - `cargo fmt`
- Tests:
  - `cargo nextest run`
- Verify generated artifacts:
  - `cargo run -p xtask -- verify-generated`
- Update semantic goldens:
  - `cargo run -p xtask -- update-snapshots`
- Full parity sweep (DOM):
  - `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3 --flowchart-text-measurer vendored`

More workflows: `docs/rendering/COMPARE_ALL_SVGS.md`.

## Reference upstreams (no submodules)

This repository uses optional local checkouts under `repo-ref/` for parity work.
These are **not committed** and are **not** git submodules.
Pinned revisions live in `repo-ref/REPOS.lock.json`.

Populate `repo-ref/*` by cloning each repo at the pinned commit shown in the lock file.

## License

Dual-licensed under MIT or Apache-2.0. See `LICENSE`, `LICENSE-MIT`, `LICENSE-APACHE`.
